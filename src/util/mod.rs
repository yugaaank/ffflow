use std::process::{Command, Stdio};
use std::time::Instant;

use crate::core::command::FfmpegCommand;
use crate::core::job::{Job, JobStatus};
use crate::core::error::FfxError;

pub mod command;
pub mod job;
pub mod progress;
pub mod error;

pub fn run(command: FfmpegCommand) -> Result<Job, FfxError> {
    
let mut job = Job {
    id:0,
    status: JobStatus::Pending,
    started_at: None,
    ended_at: None,
};

job.status = JobStatus::Running;
job.started_at = Some(Instant::now());

let mut cmd = Command::new("ffmpeg");
cmd.args(command.to_args())
    .stderr(Stdio::piped());

let mut child = cmd.spawn().map_err(|e|
    if e.kind() == std::io::ErrorKind::NotFound {
        FfxError::BinaryNotFound
    } else {
        FfxError::ProcessFailed {
            exit_code: None,
            stderr: e.to_string(),
        }
    }
})?;


let output = child.wait_with_output().map.err(|e| {
    FfxError::ProcessFailed {
        exit_code: None,
        stderr: e.to_string(),
    }
})?;

job.ended_at = Some(Instant::now());

if output.status.success() {
    job.status = JobStatus::Finished;
    Ok(job)
} else {
    job.status = JobStatus::Failed;

    Err(FfxError::ProcessFailed {
        exit_code: output.status.code(),
        stderr: String::from_utf8_lossy(&oytput.stderr).to_string(),
    })
}
}
