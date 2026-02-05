use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::core::progress::{parse_bitrate_to_kbps, parse_ffmpeg_time, parse_size_to_bytes};

#[derive(Debug, Clone, PartialEq)]
pub struct EncodeSummary {
    pub final_size_bytes: u64,
    pub duration: Duration,
    pub avg_bitrate_kbps: f32,
}

static RE_LSIZE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Lsize=\s*([0-9]*\.?[0-9]+)\s*([A-Za-z]+)").unwrap());
static RE_TIME: Lazy<Regex> = Lazy::new(|| Regex::new(r"time=\s*([0-9:\.]+)").unwrap());
static RE_BITRATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"bitrate=\s*([0-9]*\.?[0-9]+)\s*([A-Za-z/]+)").unwrap());

pub fn parse_summary_line(line: &str) -> Option<EncodeSummary> {
    let size = RE_LSIZE.captures(line).and_then(|cap| {
        let value = cap.get(1)?.as_str().parse::<f32>().ok()?;
        let unit = cap.get(2)?.as_str();
        parse_size_to_bytes(value, unit)
    });
    let duration = RE_TIME
        .captures(line)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_ffmpeg_time(m.as_str()));
    let bitrate = RE_BITRATE.captures(line).and_then(|cap| {
        let value = cap.get(1)?.as_str().parse::<f32>().ok()?;
        let unit = cap.get(2)?.as_str();
        parse_bitrate_to_kbps(value, unit)
    });

    if size.is_none() && duration.is_none() && bitrate.is_none() {
        return None;
    }

    Some(EncodeSummary {
        final_size_bytes: size.unwrap_or(0),
        duration: duration.unwrap_or(Duration::from_secs(0)),
        avg_bitrate_kbps: bitrate.unwrap_or(0.0),
    })
}
