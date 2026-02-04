#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub time: Option<String>,
    pub frame: Option<u64>,
    pub speed: Option<String>,
}

pub fn parse_progress_line(line: &str) -> Option<ProgressUpdate> {
    let mut time: Option<String> = None;
    let mut frame: Option<u64> = None;
    let mut speed: Option<String> = None;

    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("time=") {
            time = Some(value.to_string());
            continue;
        }
        if let Some(value) = token.strip_prefix("frame=") {
            if let Ok(parsed) = value.parse::<u64>() {
                frame = Some(parsed);
            }
            continue;
        }
        if let Some(value) = token.strip_prefix("speed=") {
            speed = Some(value.to_string());
            continue;
        }
    }

    if time.is_some() || frame.is_some() || speed.is_some() {
        Some(ProgressUpdate { time, frame, speed })
    } else {
        None
    }
}
