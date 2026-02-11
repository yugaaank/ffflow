use std::process::{Command, Stdio};
use std::time::Instant;

pub mod command;
pub mod error;
pub mod batch;
pub mod job;
pub mod progress;
pub mod metadata;
pub mod summary;
pub mod event;
pub mod runner;
pub mod formatter;

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

pub fn run_with_events(command: FfmpegCommand) -> (std::sync::mpsc::Receiver<event::FfmpegEvent>, std::sync::mpsc::Sender<String>) {
    runner::run_with_events(command)
}
