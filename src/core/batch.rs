use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub fn parse_flw_file(path: &Path) -> Result<Vec<String>, io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut commands = Vec::new();
    let mut current_command = String::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() && current_command.is_empty() {
            continue;
        }

        if trimmed.starts_with('#') {
            continue;
        }

        if let Some(stripped) = trimmed.strip_suffix('\\') {
            current_command.push_str(stripped.trim());
            current_command.push(' ');
        } else {
            current_command.push_str(trimmed);
            if !current_command.is_empty() {
                commands.push(current_command.clone());
                current_command.clear();
            }
        }
    }

    if !current_command.is_empty() {
        commands.push(current_command);
    }

    Ok(commands)
}
