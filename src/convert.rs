use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use png::Decoder;

use crate::metadata;

pub struct ConvertedRecord {
    pub input_file: PathBuf,
    pub input_len: usize,
    pub output_file: PathBuf,
    pub output_len: usize,
    pub output: Vec<u8>,
    pub output_metadata_text: Option<String>,
}

impl Display for ConvertedRecord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} bytes) -> {} ({} bytes)", self.input_file.display(), self.input_len, self.output_file.display(), self.output_len)
    }
}

type DynError = Box<dyn std::error::Error + Send + Sync>;

pub fn process_and_write(
    src: &Path,
    output_dir: &Path,
    input_root: &Path,
) -> Result<ConvertedRecord, DynError> {
    let res = process_file(src, output_dir, input_root)?;
    if let Some(ref t) = res.output_metadata_text {
        let txt_path = res.output_file.with_extension("webp.metadata.txt");
        fs::write(&txt_path, t)?;
    }

    fs::write(&res.output_file, &res.output)?;

    Ok(res)
}

fn process_file(
    src: &Path,
    output_dir: &Path,
    input_root: &Path,
) -> Result<ConvertedRecord, DynError> {
    // Compute output path: output_dir / relative_path_from_input_root + ".webp"
    let rel = src.strip_prefix(input_root).unwrap_or_else(|_| src.file_name().map(Path::new).unwrap_or(src));
    let webp_rel = rel.with_extension("webp");
    let out_path = output_dir.join(&webp_rel);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Read PNG bytes and decode
    let png_bytes = fs::read(src)?;

    let decoder = Decoder::new(Cursor::new(&png_bytes));
    let reader = decoder.read_info()?;
    let info = reader.info();

    let width = info.width;
    let height = info.height;

    // Write metadata text file if iTXt or zTXt chunks exist
    let has_text = !info.utf8_text.is_empty() || !info.compressed_latin1_text.is_empty();
    let txt = if has_text {
        Some(metadata::build_metadata_txt(info))
    } else {
        None
    };

    // Extract metadata
    let exif = info.exif_metadata.as_deref();
    let xmp = info.utf8_text.iter().find_map(|chunk| {
        if chunk.keyword == "XML:com.adobe.xmp" {
            chunk.get_text().ok().map(|t| t.into_bytes())
        } else {
            None
        }
    });

    // Build extended WebP with metadata
    let image = image::load_from_memory(&png_bytes)?;
    let encoder = webp::Encoder::from_image(&image)?;
    let webp_data = encoder.encode_lossless();
    let final_webp = metadata::build_extended_webp(&webp_data, width, height, exif, xmp.as_deref())?;

    Ok(ConvertedRecord {
        input_len: png_bytes.len(),
        input_file: src.to_owned(),
        output_file: out_path,
        output_len: final_webp.len(),
        output: final_webp,
        output_metadata_text: txt,
    })
}
