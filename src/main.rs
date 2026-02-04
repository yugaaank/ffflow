mod cli;
mod core;
mod tui;

fn main() {
    if let Err(err) = tui::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
