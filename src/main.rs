#![deny(clippy::all)]
#![warn(clippy::nursery)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
use clap::Parser;
use rayon::prelude::*;
use walkdir::WalkDir;
use crate::cli::Args;
use crate::console::{Command, ConsoleMessage};

mod cli;
mod console;
mod convert;
mod metadata;


fn main() -> ExitCode {
    let args = Args::parse();
    let (console_channel_sender, console_channel_writer) = std::sync::mpsc::channel::<Command<ConsoleMessage>>();

    let j = std::thread::spawn(move || {
        let print = |r: ConsoleMessage| {
            match r {
                ConsoleMessage::Error { e } => {
                    eprintln!("{e:?}");
                }
                ConsoleMessage::Processed { record } => {
                    println!("{record}");
                }
            }
        };

        loop {
            match console_channel_writer.recv_timeout(Duration::from_millis(10)) {
                Ok(convert) => {
                    match convert {
                        Command::Shutdown { graceful } => {
                            if !graceful {
                                break;
                            }

                            while let Ok(Command::Message(r)) = console_channel_writer.recv() {
                                print(r);
                            }
                        }
                        Command::Message(v) => print(v)
                    }
                }
                Err(e) => {
                    match e {
                        RecvTimeoutError::Timeout => {}
                        RecvTimeoutError::Disconnected => break
                    }
                }
            }
        }
    });

    let blocks_until_all = || {
        console_channel_sender.send(Command::Shutdown { graceful: true }).expect("send");
        j.join().expect("join");
    };

    let files: Vec<PathBuf> = if args.input.is_dir() {
        if !args.recursive {
            console_channel_sender.send(Command::Message(ConsoleMessage::Error {
                e: "recursive is not set".into()
            })).unwrap_or_default();
            blocks_until_all();

            return ExitCode::FAILURE;
        }
        WalkDir::new(&args.input)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("png")
            })
            .map(|e| e.path().to_path_buf())
            .collect()
    } else {
        vec![args.input.clone()]
    };

    let input_root = if args.input.is_dir() {
        &args.input
    } else {
        args.input.parent().unwrap_or(std::path::Path::new("."))
    };

    files.par_iter().for_each(|src| {
        match convert::process_and_write(src, &args.output, input_root) {
            Ok(convert) => {
                console_channel_sender.send(Command::Message(ConsoleMessage::Processed { record: convert })).expect("send");
            }
            Err(e) => {
                console_channel_sender.send(Command::Message(ConsoleMessage::Error { e })).expect("send");
            }
        }
    });
    
    blocks_until_all();

    ExitCode::SUCCESS
}
