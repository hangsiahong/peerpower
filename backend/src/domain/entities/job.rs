use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::shared::types::MessageStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub message_id: String,
    pub provider_id: String,
    pub status: JobStatus,
    pub assigned_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub timeout_at: DateTime<Utc>,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub fcm_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Assigned,
    Dispatched,
    InProgress,
    Completed,
    Failed,
    Timeout,
    Cancelled,
}

impl Job {
    pub fn new(message_id: String, provider_id: String) -> Self {
        let now = crate::shared::utils::now();
        Self {
            id: crate::shared::utils::generate_id(),
            message_id,
            provider_id,
            status: JobStatus::Assigned,
            assigned_at: now,
            started_at: None,
            completed_at: None,
            timeout_at: now + chrono::Duration::minutes(10), // 10 minute timeout
            retry_count: 0,
            error_message: None,
            fcm_message_id: None,
        }
    }

    pub fn mark_dispatched(&mut self, fcm_message_id: String) {
        self.status = JobStatus::Dispatched;
        self.fcm_message_id = Some(fcm_message_id);
    }

    pub fn mark_in_progress(&mut self) {
        self.status = JobStatus::InProgress;
        self.started_at = Some(crate::shared::utils::now());
    }

    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(crate::shared::utils::now());
    }

    pub fn mark_failed(&mut self, error_message: String) {
        self.status = JobStatus::Failed;
        self.error_message = Some(error_message);
        self.completed_at = Some(crate::shared::utils::now());
    }

    pub fn mark_timeout(&mut self) {
        self.status = JobStatus::Timeout;
        self.error_message = Some("Job execution timeout".to_string());
        self.completed_at = Some(crate::shared::utils::now());
    }

    pub fn is_expired(&self) -> bool {
        crate::shared::utils::now() > self.timeout_at
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Assigned | JobStatus::Dispatched | JobStatus::InProgress)
    }

    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobStatus::Failed | JobStatus::Timeout) 
            && self.retry_count < 3
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.status = JobStatus::Assigned;
        self.error_message = None;
        self.timeout_at = crate::shared::utils::now() + chrono::Duration::minutes(10);
    }
}
