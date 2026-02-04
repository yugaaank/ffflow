use thiserror::Error;

#[derive(Debug, Error)]
pub enum FfxError {
    #[error("ffmpeg binary not found in PATH")]
    BinaryNotFound,
    #[error("ffmpeg process failed (exit_code={exit_code:?}): {stderr}")]
    ProcessFailed {
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("invalid command: {message}")]
    InvalidCommand { message: String },
}
