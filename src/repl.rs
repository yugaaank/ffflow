use std::io::{self, Write};

use clap::Parser;

use crate::cli::{execute, Cli, Commands};
use crate::core::error::FfxError;

pub fn run() -> Result<(), FfxError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        print!("ffflow> ");
        stdout
            .flush()
            .map_err(|e| FfxError::InvalidCommand {
                message: e.to_string(),
            })?;

        let bytes_read = stdin
            .read_line(&mut line)
            .map_err(|e| FfxError::InvalidCommand {
                message: e.to_string(),
            })?;

        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
            break;
        }

        let mut argv = Vec::new();
        argv.push("ffflow".to_string());
        argv.extend(trimmed.split_whitespace().map(|s| s.to_string()));

        match Cli::try_parse_from(argv) {
            Ok(parsed) => match parsed.command {
                Commands::Repl => {
                    eprintln!("Already in REPL.");
                }
                other => {
                    if let Err(err) = execute(other) {
                        eprintln!("{err}");
                    }
                }
            },
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    Ok(())
}
