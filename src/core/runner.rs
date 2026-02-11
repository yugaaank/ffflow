use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::core::command::FfmpegCommand;
use crate::core::event::{classify_log_line, FfmpegEvent, LogLevel};
use crate::core::metadata::MetadataParser;
use crate::core::progress::{parse_bitrate_to_kbps, parse_ffmpeg_time, parse_progress_line, FfmpegProgress};
use crate::core::summary::parse_summary_line;

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Default)]
struct ProgressAccumulator {
    frame: Option<u64>,
    fps: Option<f32>,
    time: Option<Duration>,
    bitrate_kbps: Option<f32>,
    speed: Option<f32>,
    size_bytes: Option<u64>,
}

impl ProgressAccumulator {
    fn set_kv(&mut self, key: &str, value: &str) {
        match key {
            "frame" => {
                self.frame = value.trim().parse::<u64>().ok();
            }
            "fps" => {
                self.fps = value.trim().parse::<f32>().ok();
            }
            "bitrate" => {
                if let Some((num, unit)) = split_number_unit(value) {
                    if let Ok(parsed) = num.parse::<f32>() {
                        self.bitrate_kbps = parse_bitrate_to_kbps(parsed, unit);
                    }
                }
            }
            "speed" => {
                let trimmed = value.trim().trim_end_matches('x');
                self.speed = trimmed.parse::<f32>().ok();
            }
            "total_size" | "size" => {
                self.size_bytes = value.trim().parse::<u64>().ok();
            }
            "out_time" => {
                self.time = parse_ffmpeg_time(value.trim());
            }
            "out_time_ms" => {
                if let Ok(parsed) = value.trim().parse::<u64>() {
                    self.time = Some(Duration::from_micros(parsed));
                }
            }
            "out_time_us" => {
                if let Ok(parsed) = value.trim().parse::<u64>() {
                    self.time = Some(Duration::from_micros(parsed));
                }
            }
            _ => {}
        }
    }

    fn to_progress(&self) -> Option<FfmpegProgress> {
        if self.frame.is_none()
            && self.fps.is_none()
            && self.time.is_none()
            && self.bitrate_kbps.is_none()
            && self.speed.is_none()
            && self.size_bytes.is_none()
        {
            return None;
        }

        Some(FfmpegProgress {
            frame: self.frame.unwrap_or(0),
            fps: self.fps.unwrap_or(0.0),
            time: self.time.unwrap_or(Duration::from_secs(0)),
            bitrate_kbps: self.bitrate_kbps.unwrap_or(0.0),
            speed: self.speed.unwrap_or(0.0),
            size_bytes: self.size_bytes.unwrap_or(0),
        })
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn split_number_unit(value: &str) -> Option<(&str, &str)> {
    let trimmed = value.trim();
    let mut idx = 0;
    for (pos, ch) in trimmed.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.') {
            idx = pos;
            break;
        }
    }
    if idx == 0 || idx >= trimmed.len() {
        return None;
    }
    Some((&trimmed[..idx], trimmed[idx..].trim()))
}

fn has_progress_stdout(args: &[String]) -> bool {
    if args.iter().any(|arg| arg.starts_with("-progress=") && arg.contains("pipe:1")) {
        return true;
    }

    args.windows(2)
        .any(|pair| pair[0] == "-progress" && pair[1].starts_with("pipe:1"))
}

pub fn run_with_events(command: FfmpegCommand) -> (Receiver<FfmpegEvent>, Sender<String>) {
    run_args_with_events(command.to_args())
}

pub fn run_args_with_events(args: Vec<String>) -> (Receiver<FfmpegEvent>, Sender<String>) {
    let (event_tx, event_rx) = mpsc::channel::<FfmpegEvent>();
    let (stdin_tx, stdin_rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&args).stderr(Stdio::piped()).stdin(Stdio::piped());

        if has_progress_stdout(&args) {
            cmd.stdout(Stdio::piped());
        } else {
            cmd.stdout(Stdio::null());
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = event_tx.send(FfmpegEvent::Error(err.to_string()));
                return;
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            thread::spawn(move || {
                use std::io::Write;
                for input in stdin_rx {
                    if let Err(_) = stdin.write_all(input.as_bytes()) {
                        break;
                    }
                    if let Err(_) = stdin.flush() {
                        break;
                    }
                }
            });
        }

        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                let _ = event_tx.send(FfmpegEvent::Error("failed to capture ffmpeg stderr".to_string()));
                let _ = child.wait();
                return;
            }
        };

        let (line_tx, line_rx) = mpsc::channel::<(StreamKind, String)>();
        let stderr_tx = line_tx.clone();
        let stderr_handle = spawn_line_reader(StreamKind::Stderr, stderr, stderr_tx);

        let stdout_handle = if has_progress_stdout(&args) {
            if let Some(stdout) = child.stdout.take() {
                Some(spawn_line_reader(StreamKind::Stdout, stdout, line_tx.clone()))
            } else {
                None
            }
        } else {
            None
        };

        drop(line_tx);

        let mut metadata = MetadataParser::new();
        let mut progress_acc = ProgressAccumulator::default();

        for (stream, line) in line_rx {
            match stream {
                StreamKind::Stdout => {
                    if let Some(progress) = parse_progress_kv_line(&line, &mut progress_acc) {
                        let _ = event_tx.send(FfmpegEvent::Progress(progress));
                    }
                }
                StreamKind::Stderr => {
                    if let Some(progress) = parse_progress_line(&line) {
                        let _ = event_tx.send(FfmpegEvent::Progress(progress));
                        continue;
                    }

                    if let Some(input) = metadata.parse_input_line(&line) {
                        let _ = event_tx.send(FfmpegEvent::Input(input));
                        continue;
                    }

                    if let Some(output) = metadata.parse_output_line(&line) {
                        let _ = event_tx.send(FfmpegEvent::Output(output));
                        continue;
                    }

                    if let Some(summary) = parse_summary_line(&line) {
                        let _ = event_tx.send(FfmpegEvent::Summary(summary));
                        continue;
                    }

                    let level = classify_log_line(&line);
                    if matches!(level, LogLevel::Error) {
                        let _ = event_tx.send(FfmpegEvent::Error(line.clone()));
                    } else if matches!(level, LogLevel::Prompt) {
                        let _ = event_tx.send(FfmpegEvent::Prompt(line));
                    }
                }
            }
        }

        let _ = stderr_handle.join();
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }

        if let Ok(status) = child.wait() {
            if !status.success() {
                let message = format!("ffmpeg exited with status {status}");
                let _ = event_tx.send(FfmpegEvent::Error(message));
            }
        }
    });

    (event_rx, stdin_tx)
}

fn spawn_line_reader<R: Read + Send + 'static>(
    stream: StreamKind,
    reader: R,
    sender: Sender<(StreamKind, String)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line_buf: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let read = match reader.read(&mut byte) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };

            if read == 0 {
                break;
            }

            match byte[0] {
                b'\r' | b'\n' => {
                    if line_buf.is_empty() {
                        continue;
                    }
                    let line = String::from_utf8_lossy(&line_buf)
                        .trim_matches(&['\r', '\n'][..])
                        .to_string();
                    line_buf.clear();
                    if !line.is_empty() {
                        let _ = sender.send((stream, line));
                    }
                }
                other => {
                    line_buf.push(other);
                }
            }
        }

        if !line_buf.is_empty() {
            let line = String::from_utf8_lossy(&line_buf)
                .trim_matches(&['\r', '\n'][..])
                .to_string();
            if !line.is_empty() {
                let _ = sender.send((stream, line));
            }
        }
    })
}

fn parse_progress_kv_line(line: &str, acc: &mut ProgressAccumulator) -> Option<FfmpegProgress> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((key, value)) = trimmed.split_once('=') {
        if key == "progress" {
            let progress = acc.to_progress();
            acc.reset();
            return progress;
        }

        acc.set_kv(key.trim(), value.trim());
        return None;
    }

    parse_progress_line(trimmed)
}
