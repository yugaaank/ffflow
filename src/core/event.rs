use crate::core::metadata::{InputInfo, OutputInfo};
use crate::core::progress::FfmpegProgress;
use crate::core::summary::EncodeSummary;

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Progress,
    Input,
    Output,
    Summary,
    Warning,
    Error,
    Prompt,
    Noise,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FfmpegEvent {
    Progress(FfmpegProgress),
    Input(InputInfo),
    Output(OutputInfo),
    Summary(EncodeSummary),
    Error(String),
    Prompt(String),
}

pub fn classify_log_line(line: &str) -> LogLevel {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LogLevel::Noise;
    }

    if trimmed.starts_with("Input #") {
        return LogLevel::Input;
    }
    if trimmed.starts_with("Output #") {
        return LogLevel::Output;
    }
    if trimmed.contains("frame=") && trimmed.contains("time=") {
        return LogLevel::Progress;
    }
    if trimmed.contains("Lsize=") && trimmed.contains("bitrate=") {
        return LogLevel::Summary;
    }

    if trimmed.contains("Overwrite?") && trimmed.contains("[y/N]") {
        return LogLevel::Prompt;
    }

    let lower = trimmed.to_ascii_lowercase();
    let noise_prefixes = [
        "ffmpeg version",
        "built with",
        "configuration:",
        "libavutil",
        "libavcodec",
        "libavformat",
        "libavdevice",
        "libavfilter",
        "libswscale",
        "libswresample",
        "libpostproc",
        "cpu capabilities",
        "using cpu capabilities",
    ];

    if noise_prefixes.iter().any(|prefix| lower.starts_with(prefix)) {
        return LogLevel::Noise;
    }

    let noise_contains = [
        "x264 [info]:",
        "x265 [info]:",
        "cabac",
        "qp",
        "mb ",
        "psy",
        "sse2",
        "sse4",
        "avx",
        "mmx",
        "cpu flags",
        "profile high",
    ];

    if noise_contains.iter().any(|needle| lower.contains(needle)) {
        return LogLevel::Noise;
    }

    if lower.contains("error") || lower.contains("invalid") || lower.contains("no such file") {
        return LogLevel::Error;
    }

    if lower.contains("warning") || lower.contains("deprecated") {
        return LogLevel::Warning;
    }

    LogLevel::Noise
}
