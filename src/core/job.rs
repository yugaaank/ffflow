use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Finished,
    Failed,
    AwaitingConfirmation,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub status: JobStatus,
    pub started_at: Option<Instant>,
    pub ended_at: Option<Instant>,
}
