use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::core::progress::parse_ffmpeg_time;

#[derive(Debug, Clone, PartialEq)]
pub struct InputInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub codec: String,
    pub duration: Option<Duration>,
    pub container: Option<String>,
    pub path: Option<String>,
    pub bitrate_kbps: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub container: String,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

static RE_INPUT_HEADER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Input #\d+,\s*(.+),\s*from '([^']+)'").unwrap());
static RE_OUTPUT_HEADER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Output #\d+,\s*([^,]+),\s*to '([^']+)'").unwrap());
static RE_DURATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Duration:\s*([0-9:\.]+)").unwrap());
static RE_BITRATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"bitrate:\s*([0-9]*\.?[0-9]+)\s*kb/s").unwrap());
static RE_STREAM_VIDEO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Stream #\d+:\d+.*Video:\s*([^,]+)").unwrap());
static RE_RESOLUTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d{2,5})x(\d{2,5})").unwrap());
static RE_FPS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([0-9]*\.?[0-9]+)\s*fps").unwrap());

#[derive(Default)]
pub struct MetadataParser {
    pending_input_duration: Option<Duration>,
    pending_input_container: Option<String>,
    pending_input_path: Option<String>,
    pending_input_bitrate_kbps: Option<f32>,
    input_emitted: bool,
    pending_output_container: Option<String>,
    pending_output_path: Option<String>,
    section: MetadataSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataSection {
    Input,
    Output,
    Other,
}

impl Default for MetadataSection {
    fn default() -> Self {
        MetadataSection::Other
    }
}

impl MetadataParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_input_line(&mut self, line: &str) -> Option<InputInfo> {
        if let Some(capture) = RE_INPUT_HEADER.captures(line) {
            let container = capture.get(1).map(|m| m.as_str().trim().to_string());
            let path = capture.get(2).map(|m| m.as_str().trim().to_string());
            self.pending_input_container = container;
            self.pending_input_path = path;
            self.pending_input_duration = None;
            self.pending_input_bitrate_kbps = None;
            self.input_emitted = false;
            self.section = MetadataSection::Input;
            return None;
        }

        if RE_OUTPUT_HEADER.is_match(line) {
            self.section = MetadataSection::Output;
            if !self.input_emitted {
                if let Some(info) = self.build_input_info(None, 0, 0, 0.0) {
                    self.input_emitted = true;
                    return Some(info);
                }
            }
            return None;
        }

        if self.section != MetadataSection::Input {
            return None;
        }

        if let Some(capture) = RE_DURATION.captures(line) {
            if let Some(value) = capture.get(1) {
                self.pending_input_duration = parse_ffmpeg_time(value.as_str());
            }
            if let Some(bitrate_cap) = RE_BITRATE.captures(line) {
                if let Some(value) = bitrate_cap.get(1) {
                    self.pending_input_bitrate_kbps = value.as_str().parse::<f32>().ok();
                }
            }
            return None;
        }

        if self.input_emitted {
            return None;
        }

        let codec = RE_STREAM_VIDEO
            .captures(line)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string());

        let (width, height) = RE_RESOLUTION
            .captures(line)
            .and_then(|cap| {
                let w = cap.get(1)?.as_str().parse::<u32>().ok()?;
                let h = cap.get(2)?.as_str().parse::<u32>().ok()?;
                Some((w, h))
            })
            .unwrap_or((0, 0));

        let fps = RE_FPS
            .captures(line)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<f32>().ok())
            .unwrap_or(0.0);

        if codec.is_none() && width == 0 && height == 0 && fps == 0.0 {
            return None;
        }

        let info = self.build_input_info(codec, width, height, fps);
        if info.is_some() {
            self.input_emitted = true;
        }
        info
    }

    fn build_input_info(
        &self,
        codec: Option<String>,
        width: u32,
        height: u32,
        fps: f32,
    ) -> Option<InputInfo> {
        if codec.is_none()
            && width == 0
            && height == 0
            && fps == 0.0
            && self.pending_input_container.is_none()
            && self.pending_input_path.is_none()
            && self.pending_input_duration.is_none()
            && self.pending_input_bitrate_kbps.is_none()
        {
            return None;
        }

        Some(InputInfo {
            width,
            height,
            fps,
            codec: codec.unwrap_or_default(),
            duration: self.pending_input_duration,
            container: self.pending_input_container.clone(),
            path: self.pending_input_path.clone(),
            bitrate_kbps: self.pending_input_bitrate_kbps,
        })
    }

    pub fn parse_output_line(&mut self, line: &str) -> Option<OutputInfo> {
        if let Some(capture) = RE_OUTPUT_HEADER.captures(line) {
            let container = capture.get(1).map(|m| m.as_str().trim().to_string());
            let path = capture.get(2).map(|m| m.as_str().trim().to_string());
            self.pending_output_container = container;
            self.pending_output_path = path;
            self.section = MetadataSection::Output;
            return None;
        }

        if self.section != MetadataSection::Output && self.pending_output_container.is_none() {
            return None;
        }

        let codec = RE_STREAM_VIDEO
            .captures(line)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string());
        if codec.is_none() {
            return None;
        }

        let (width, height) = RE_RESOLUTION
            .captures(line)
            .and_then(|cap| {
                let w = cap.get(1)?.as_str().parse::<u32>().ok()?;
                let h = cap.get(2)?.as_str().parse::<u32>().ok()?;
                Some((w, h))
            })
            .unwrap_or((0, 0));

        let container = self.pending_output_container.take().unwrap_or_default();
        let path = self.pending_output_path.take().unwrap_or_default();

        Some(OutputInfo {
            container,
            codec: codec.unwrap_or_default(),
            width,
            height,
            path,
        })
    }
}
