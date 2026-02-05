use std::time::Duration;

use crate::core::metadata::{InputInfo, OutputInfo};
use crate::core::progress::FfmpegProgress;
use crate::core::summary::EncodeSummary;

pub fn format_input_line(info: &InputInfo) -> String {
    let resolution = if info.width > 0 && info.height > 0 {
        format!("{}x{}", info.width, info.height)
    } else {
        "unknown".to_string()
    };
    let fps = if info.fps > 0.0 {
        format!("{:.2}fps", info.fps)
    } else {
        "unknown fps".to_string()
    };
    let codec = if info.codec.is_empty() {
        "unknown".to_string()
    } else {
        info.codec.clone()
    };
    let container = info
        .container
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let path = info.path.clone().unwrap_or_else(|| "unknown".to_string());
    let duration = info
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--:--".to_string());
    let bitrate = info
        .bitrate_kbps
        .map(|kbps| format!("{:.1} kb/s", kbps))
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Input  : {path} ({container}/{codec} {resolution} @ {fps}, duration={duration}, bitrate={bitrate})"
    )
}

pub fn format_output_line(info: &OutputInfo) -> String {
    let resolution = if info.width > 0 && info.height > 0 {
        format!("{}x{}", info.width, info.height)
    } else {
        "unknown".to_string()
    };
    let codec = if info.codec.is_empty() {
        "unknown".to_string()
    } else {
        info.codec.clone()
    };
    let container = if info.container.is_empty() {
        "unknown".to_string()
    } else {
        info.container.clone()
    };
    let path = if info.path.is_empty() {
        "output".to_string()
    } else {
        info.path.clone()
    };
    format!("Output : {path} ({container}/{codec} {resolution})")
}

pub fn format_summary_line(summary: &EncodeSummary) -> String {
    let size = format_bytes(summary.final_size_bytes);
    let bitrate = if summary.avg_bitrate_kbps > 0.0 {
        format!("{:.1} kbps", summary.avg_bitrate_kbps)
    } else {
        "unknown".to_string()
    };
    let duration = format_duration(summary.duration);
    format!("Final  : size={size} avg_bitrate={bitrate} duration={duration}")
}

pub fn format_progress_line(update: &FfmpegProgress, total: Option<Duration>) -> Option<String> {
    if update.frame == 0 && update.speed == 0.0 && update.time == Duration::from_secs(0) {
        return None;
    }

    let elapsed = format_duration(update.time);
    let total = total
        .map(format_duration)
        .unwrap_or_else(|| "--:--:--".to_string());

    Some(format!(
        "progress: time={elapsed}/{total} frame={} speed={}x",
        update.frame,
        update.speed
    ))
}

pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.2} MB", value / MB)
    } else if value >= KB {
        format!("{:.2} KB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}
