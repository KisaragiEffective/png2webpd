use png::Info;

/// Build an Extended WebP RIFF container embedding EXIF and/or XMP alongside the VP8L bitstream.
///
/// `vp8l_webp` is the raw output of `webp::Encoder::encode_lossless()` — a complete
/// RIFF/WEBP file containing a single VP8L chunk.
pub fn build_extended_webp(
    vp8l_webp: &[u8],
    width: u32,
    height: u32,
    exif: Option<&[u8]>,
    xmp: Option<&[u8]>,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // If no metadata, return the original WebP as-is
    if exif.is_none() && xmp.is_none() {
        return Ok(vp8l_webp.to_vec());
    }

    // Extract VP8L payload from the input RIFF/WEBP container
    // Structure: "RIFF" <4-byte LE size> "WEBP" <chunks...>
    // We look for the VP8L chunk inside
    let vp8l_payload = extract_chunk(vp8l_webp, b"VP8L")
        .ok_or("VP8L chunk not found in webp encoder output")?;

    let has_exif = exif.is_some();
    let has_xmp = xmp.is_some();

    // VP8X flags: bit 2 = EXIF present, bit 3 = XMP present, bit 4 = animation (not set)
    let mut flags: u32 = 0;
    if has_exif {
        flags |= 1 << 2;
    }
    if has_xmp {
        flags |= 1 << 3;
    }

    // Canvas width/height stored as (value - 1) in 3 bytes LE
    let canvas_w = width - 1;
    let canvas_h = height - 1;

    let mut body: Vec<u8> = Vec::new();

    // VP8X chunk: 10 bytes payload
    // [flags: 4 bytes LE][canvas_w: 3 bytes LE][canvas_h: 3 bytes LE]
    let mut vp8x_payload = Vec::with_capacity(10);
    vp8x_payload.extend_from_slice(&flags.to_le_bytes());
    vp8x_payload.push((canvas_w & 0xFF) as u8);
    vp8x_payload.push(((canvas_w >> 8) & 0xFF) as u8);
    vp8x_payload.push(((canvas_w >> 16) & 0xFF) as u8);
    vp8x_payload.push((canvas_h & 0xFF) as u8);
    vp8x_payload.push(((canvas_h >> 8) & 0xFF) as u8);
    vp8x_payload.push(((canvas_h >> 16) & 0xFF) as u8);
    write_chunk(&mut body, b"VP8X", &vp8x_payload);

    // EXIF chunk (if present)
    if let Some(exif_data) = exif {
        write_chunk(&mut body, b"EXIF", &exif_data);
    }

    // XMP chunk (if present)
    if let Some(xmp_data) = xmp {
        write_chunk(&mut body, b"XMP ", &xmp_data);
    }

    // VP8L chunk
    write_chunk(&mut body, b"VP8L", vp8l_payload);

    // Wrap in RIFF/WEBP container
    let mut out = Vec::with_capacity(12 + body.len());
    out.extend_from_slice(b"RIFF");
    // RIFF size = 4 (for "WEBP") + body length
    let riff_size = (4 + body.len()) as u32;
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);

    Ok(out)
}

/// Write a RIFF chunk: FourCC (4 bytes) + size (4 bytes LE) + data + optional padding byte.
fn write_chunk(buf: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]) {
    buf.extend_from_slice(fourcc);
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
    if !data.len().is_multiple_of(2) {
        buf.push(0); // padding
    }
}

/// Find and return the payload of the first chunk with the given FourCC inside a RIFF/WEBP file.
fn extract_chunk<'a>(riff: &'a [u8], fourcc: &[u8; 4]) -> Option<&'a [u8]> {
    // Skip "RIFF" (4) + size (4) + "WEBP" (4) = 12 bytes header
    if riff.len() < 12 {
        return None;
    }
    let mut pos = 12usize;
    while pos + 8 <= riff.len() {
        let tag = &riff[pos..pos + 4];
        let size = u32::from_le_bytes(riff[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        if tag == fourcc {
            return riff.get(pos..pos + size);
        }
        // Advance past chunk data (with padding)
        pos += size + (size % 2);
    }
    None
}

/// Build a human-readable text file from iTXt and zTXt PNG chunks.
pub fn build_metadata_txt(info: &Info) -> String {
    let mut out = String::new();

    for chunk in &info.utf8_text {
        out.push('[');
        out.push_str(&chunk.keyword);
        out.push_str("]\n");
        if let Ok(text) = chunk.get_text() {
            out.push_str(&text);
        }
        out.push_str("\n\n");
    }

    for chunk in &info.compressed_latin1_text {
        out.push('[');
        out.push_str(&chunk.keyword);
        out.push_str("]\n");
        if let Ok(text) = chunk.get_text() {
            out.push_str(&text);
        }
        out.push_str("\n\n");
    }

    out
}
