use std::process::{Command, Stdio};
use std::time::Instant;

pub mod command;
pub mod error;
pub mod job;
pub mod progress;

use command::FfmpegCommand;
use error::FfxError;
use job::{Job, JobStatus};

pub fn run(command: FfmpegCommand) -> Result<Job, FfxError> {
    let mut job = Job {
        id: 1,
        status: JobStatus::Pending,
        started_at: None,
        ended_at: None,
    };

    job.status = JobStatus::Running;
    job.started_at = Some(Instant::now());

    let mut cmd = Command::new("ffmpeg");
    cmd.args(command.to_args()).stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FfxError::BinaryNotFound
        } else {
            FfxError::ProcessFailed {
                exit_code: None,
                stderr: e.to_string(),
            }
        }
    })?;

    let output = child.wait_with_output().map_err(|e| FfxError::ProcessFailed {
        exit_code: None,
        stderr: e.to_string(),
    })?;

    job.ended_at = Some(Instant::now());

    if output.status.success() {
        job.status = JobStatus::Finished;
        Ok(job)
    } else {
        Err(FfxError::ProcessFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn run_with_progress(
    command: FfmpegCommand,
    progress_tx: std::sync::mpsc::Sender<progress::ProgressUpdate>,
    log_tx: Option<std::sync::mpsc::Sender<String>>,
) -> Result<Job, FfxError> {
    run_args_with_progress(command.to_args(), progress_tx, log_tx)
}

pub fn run_args_with_progress(
    args: Vec<String>,
    progress_tx: std::sync::mpsc::Sender<progress::ProgressUpdate>,
    log_tx: Option<std::sync::mpsc::Sender<String>>,
) -> Result<Job, FfxError> {
    use std::io::{BufReader, Read};
    use std::sync::{Arc, Mutex};
    use std::thread;

    let mut job = Job {
        id: 1,
        status: JobStatus::Pending,
        started_at: None,
        ended_at: None,
    };

    job.status = JobStatus::Running;
    job.started_at = Some(Instant::now());

    let mut cmd = Command::new("ffmpeg");
    cmd.args(args).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FfxError::BinaryNotFound
        } else {
            FfxError::ProcessFailed {
                exit_code: None,
                stderr: e.to_string(),
            }
        }
    })?;

    let stderr = child.stderr.take().ok_or_else(|| FfxError::ProcessFailed {
        exit_code: None,
        stderr: "failed to capture ffmpeg stderr".to_string(),
    })?;

    let stderr_buffer = Arc::new(Mutex::new(String::new()));
    let stderr_buffer_reader = Arc::clone(&stderr_buffer);

    let reader_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
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

                    if line.is_empty() {
                        continue;
                    }

                    if let Some(sender) = &log_tx {
                        let _ = sender.send(line.clone());
                    }

                    if let Some(update) = progress::parse_progress_line(&line) {
                        let _ = progress_tx.send(update);
                    }

                    if let Ok(mut buffer) = stderr_buffer_reader.lock() {
                        buffer.push_str(&line);
                        buffer.push('\n');
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
                if let Some(sender) = &log_tx {
                    let _ = sender.send(line.clone());
                }

                if let Some(update) = progress::parse_progress_line(&line) {
                    let _ = progress_tx.send(update);
                }

                if let Ok(mut buffer) = stderr_buffer_reader.lock() {
                    buffer.push_str(&line);
                    buffer.push('\n');
                }
            }
        }
    });

    let status = child.wait().map_err(|e| FfxError::ProcessFailed {
        exit_code: None,
        stderr: e.to_string(),
    })?;

    let _ = reader_handle.join();

    job.ended_at = Some(Instant::now());

    if status.success() {
        job.status = JobStatus::Finished;
        Ok(job)
    } else {
        job.status = JobStatus::Failed;
        let stderr = stderr_buffer
            .lock()
            .map(|buffer| buffer.clone())
            .unwrap_or_else(|_| "failed to read stderr buffer".to_string());
        Err(FfxError::ProcessFailed {
            exit_code: status.code(),
            stderr,
        })
    }
}
