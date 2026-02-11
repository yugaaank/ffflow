mod cli;
mod core;
mod tui;

use clap::Parser;
use cli::SystemCli;
use core::batch;

fn main() {
    let args = SystemCli::parse();
    let mut queue = Vec::new();

    if let Some(path) = args.file {
        match batch::parse_flw_file(&path) {
            Ok(cmds) => queue = cmds,
            Err(e) => {
                eprintln!("Error reading batch file: {}", e);
                std::process::exit(1);
            }
        }
    }

    if let Err(err) = tui::run(queue) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
