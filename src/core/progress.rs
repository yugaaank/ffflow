use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct FfmpegProgress {
    pub frame: u64,
    pub fps: f32,
    pub time: Duration,
    pub bitrate_kbps: f32,
    pub speed: f32,
    pub size_bytes: u64,
}

static RE_FRAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"frame=\s*(\d+)").unwrap());
static RE_FPS: Lazy<Regex> = Lazy::new(|| Regex::new(r"fps=\s*([0-9]*\.?[0-9]+)").unwrap());
static RE_TIME: Lazy<Regex> = Lazy::new(|| Regex::new(r"time=\s*([0-9:\.]+)").unwrap());
static RE_BITRATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"bitrate=\s*([0-9]*\.?[0-9]+)\s*([A-Za-z/]+)").unwrap());
static RE_SPEED: Lazy<Regex> = Lazy::new(|| Regex::new(r"speed=\s*([0-9]*\.?[0-9]+)x").unwrap());
static RE_SIZE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"size=\s*([0-9]*\.?[0-9]+)\s*([A-Za-z]+)").unwrap());

pub fn parse_progress_line(line: &str) -> Option<FfmpegProgress> {
    let frame = RE_FRAME
        .captures(line)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok());
    let fps = RE_FPS
        .captures(line)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok());
    let time = RE_TIME
        .captures(line)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_ffmpeg_time(m.as_str()));
    let bitrate = RE_BITRATE.captures(line).and_then(|cap| {
        let value = cap.get(1)?.as_str().parse::<f32>().ok()?;
        let unit = cap.get(2)?.as_str();
        parse_bitrate_to_kbps(value, unit)
    });
    let speed = RE_SPEED
        .captures(line)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok());
    let size_bytes = RE_SIZE.captures(line).and_then(|cap| {
        let value = cap.get(1)?.as_str().parse::<f32>().ok()?;
        let unit = cap.get(2)?.as_str();
        parse_size_to_bytes(value, unit)
    });

    if frame.is_none()
        && fps.is_none()
        && time.is_none()
        && bitrate.is_none()
        && speed.is_none()
        && size_bytes.is_none()
    {
        return None;
    }

    Some(FfmpegProgress {
        frame: frame.unwrap_or(0),
        fps: fps.unwrap_or(0.0),
        time: time.unwrap_or(Duration::from_secs(0)),
        bitrate_kbps: bitrate.unwrap_or(0.0),
        speed: speed.unwrap_or(0.0),
        size_bytes: size_bytes.unwrap_or(0),
    })
}

pub fn parse_ffmpeg_time(value: &str) -> Option<Duration> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.is_empty() {
        return None;
    }

    let seconds = match parts.len() {
        1 => parts[0].parse::<f64>().ok()?,
        2 => {
            let minutes = parts[0].parse::<f64>().ok()?;
            let seconds = parts[1].parse::<f64>().ok()?;
            minutes * 60.0 + seconds
        }
        _ => {
            let hours = parts[0].parse::<f64>().ok()?;
            let minutes = parts[1].parse::<f64>().ok()?;
            let seconds = parts[2].parse::<f64>().ok()?;
            hours * 3600.0 + minutes * 60.0 + seconds
        }
    };

    let micros = (seconds * 1_000_000.0).round().max(0.0) as u64;
    Some(Duration::from_micros(micros))
}

pub fn parse_size_to_bytes(value: f32, unit: &str) -> Option<u64> {
    let unit = unit.trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "b" | "bytes" => 1.0,
        "kb" => 1000.0,
        "kib" => 1024.0,
        "mb" => 1_000_000.0,
        "mib" => 1_048_576.0,
        "gb" => 1_000_000_000.0,
        "gib" => 1_073_741_824.0,
        "tb" => 1_000_000_000_000.0,
        "tib" => 1_099_511_627_776.0,
        _ => return None,
    };
    Some((value as f64 * multiplier).round().max(0.0) as u64)
}

pub fn parse_bitrate_to_kbps(value: f32, unit: &str) -> Option<f32> {
    let unit = unit.trim().to_ascii_lowercase();
    let multiplier = if unit.starts_with("kbit") {
        1.0
    } else if unit.starts_with("mbit") {
        1000.0
    } else if unit.starts_with("gbit") {
        1_000_000.0
    } else {
        return None;
    };
    Some(value * multiplier)
}
