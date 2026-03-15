use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
#[command(about = "Convert PNG files to lossless WebP, preserving metadata")]
pub struct Args {
    pub input: PathBuf,
    pub output: PathBuf,
    #[arg(short, long)]
    pub recursive: bool,
}