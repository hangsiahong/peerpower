use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

use crate::domain::entities::{Job, Message, Provider};
use crate::infrastructure::messaging::fcm_service::FcmService;
use crate::shared::types::{MessageStatus, ProviderStatus};
use crate::shared::{AppState, PeerPowerError, Result};

/// Job processor service that handles the job queue
pub struct JobProcessor {
    app_state: Arc<AppState>,
    is_running: bool,
}

impl JobProcessor {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            app_state,
            is_running: false,
        }
    }

    /// Start the job processor
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            warn!("Job processor is already running");
            return Ok(());
        }

        self.is_running = true;
        info!("Starting job processor...");

        // Start the main processing loop
        let app_state = self.app_state.clone();
        tokio::spawn(async move {
            Self::process_jobs_loop(app_state).await;
        });

        // Start the cleanup task
        let app_state = self.app_state.clone();
        tokio::spawn(async move {
            Self::cleanup_expired_jobs_loop(app_state).await;
        });

        info!("Job processor started successfully");
        Ok(())
    }

    /// Main job processing loop
    async fn process_jobs_loop(app_state: Arc<AppState>) {
        let mut interval = interval(Duration::from_secs(5)); // Process every 5 seconds

        loop {
            interval.tick().await;

            if let Err(e) = Self::process_pending_jobs(&app_state).await {
                error!("Error processing jobs: {}", e);
                sleep(Duration::from_secs(10)).await; // Back off on error
            }
        }
    }

    /// Process pending jobs from the queue
    async fn process_pending_jobs(app_state: &Arc<AppState>) -> Result<()> {
        // Get high priority jobs first
        let priority_queues = [
            "jobs:queue:priority:3",
            "jobs:queue:priority:2",
            "jobs:queue:priority:1",
            "jobs:queue:priority:0",
        ];

        for queue_key in &priority_queues {
            // Try to get a job from this priority queue
            if let Some(job_data) = app_state.redis.rpop(queue_key).await? {
                match serde_json::from_str::<Job>(&job_data) {
                    Ok(job) => {
                        info!("Processing job: {}", job.id);
                        if let Err(e) = Self::process_single_job(app_state, job).await {
                            error!("Failed to process job: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize job: {}", e);
                    }
                }
                break; // Process one job at a time
            }
        }

        Ok(())
    }

    /// Process a single job
    async fn process_single_job(app_state: &Arc<AppState>, mut job: Job) -> Result<()> {
        // Get the message details
        let messages_collection = app_state.database.collection::<Message>("messages");
        let mut message = messages_collection
            .find_one(mongodb::bson::doc! {"id": &job.message_id}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to fetch message: {}", e),
            })?
            .ok_or_else(|| PeerPowerError::NotFound {
                resource: format!("Message: {}", job.message_id),
            })?;

        // Find an available provider
        match Self::find_available_provider(app_state, &message).await? {
            Some(provider) => {
                info!("Assigned job {} to provider {}", job.id, provider.id);

                // Update message and job status
                message.provider_id = Some(provider.id.clone());
                message.status = MessageStatus::Assigned;

                job.provider_id = provider.id.clone();
                job.mark_dispatched("fcm_message_id".to_string()); // TODO: Get actual FCM message ID

                // Send FCM notification to provider
                match Self::send_fcm_notification(app_state, &job, &message, &provider).await {
                    Ok(_) => {
                        info!("FCM notification sent for job {}", job.id);
                        message.status = MessageStatus::Sent;
                    }
                    Err(e) => {
                        error!("Failed to send FCM notification: {}", e);
                        message.status = MessageStatus::Failed;
                        job.mark_failed(format!("FCM failed: {}", e));

                        // Re-queue for retry if possible
                        if job.can_retry() {
                            Self::requeue_job(app_state, &job).await?;
                        }
                    }
                }

                // Update database
                Self::update_message_and_job(app_state, &message, &job).await?;
            }
            None => {
                info!("No available provider for job {}, re-queuing", job.id);

                // Re-queue the job for later processing
                Self::requeue_job(app_state, &job).await?;
            }
        }

        Ok(())
    }

    /// Find an available provider for the message
    async fn find_available_provider(
        app_state: &Arc<AppState>,
        message: &Message,
    ) -> Result<Option<Provider>> {
        let providers_collection = app_state.database.collection::<Provider>("providers");

        // Try to find a provider with the same carrier as recipient (for better delivery rates)
        let target_carrier = crate::shared::types::Carrier::from_phone_number(&message.recipient);

        let mut cursor = providers_collection
            .find(
                mongodb::bson::doc! {
                    "carrier": format!("{:?}", target_carrier),
                    "status": format!("{:?}", ProviderStatus::Online),
                    "current_load": {"$lt": 5}, // Not overloaded
                    "messages_sent_today": {"$lt": mongodb::bson::doc!{"$field": "max_daily_messages"}},
                },
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to query providers: {}", e),
            })?;

        // Try to get the first available provider
        if let Some(provider) = cursor
            .try_next()
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to fetch provider: {}", e),
            })?
        {
            return Ok(Some(provider));
        }

        // If no same-carrier provider available, try any available provider
        let mut cursor = providers_collection
            .find(
                mongodb::bson::doc! {
                    "status": format!("{:?}", ProviderStatus::Online),
                    "current_load": {"$lt": 5},
                    "messages_sent_today": {"$lt": mongodb::bson::doc!{"$field": "max_daily_messages"}},
                },
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to query providers: {}", e),
            })?;

        if let Some(provider) = cursor
            .try_next()
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to fetch provider: {}", e),
            })?
        {
            return Ok(Some(provider));
        }

        Ok(None)
    }

    /// Send FCM notification to provider device
    async fn send_fcm_notification(
        app_state: &Arc<AppState>,
        job: &Job,
        message: &Message,
        provider: &Provider,
    ) -> Result<()> {
        let fcm_token =
            provider
                .fcm_token
                .as_ref()
                .ok_or_else(|| PeerPowerError::ValidationError {
                    field: "fcm_token".to_string(),
                    message: "Provider has no FCM token".to_string(),
                })?;

        info!(
            "Sending FCM dispatch request to provider {} for message {}",
            provider.id, message.id
        );

        // Send SMS dispatch request via FCM
        let result = app_state
            .fcm_service
            .send_sms_dispatch_request(
                fcm_token,
                &message.id,
                message.recipient.as_str(),
                &message.content,
                &format!("{:?}", message.priority),
            )
            .await;

        match result {
            Ok(response) => {
                info!("FCM dispatch request sent successfully: {}", response);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send FCM dispatch request: {}", e);
                Err(e)
            }
        }
    }

    /// Re-queue a job for retry
    async fn requeue_job(app_state: &Arc<AppState>, job: &Job) -> Result<()> {
        // Add delay before retrying (exponential backoff)
        let delay_seconds = 2_u64.pow(job.retry_count.min(6)); // Max 64 seconds delay

        tokio::spawn({
            let app_state = app_state.clone();
            let job = job.clone();
            async move {
                sleep(Duration::from_secs(delay_seconds)).await;

                let job_data = match serde_json::to_string(&job) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to serialize job for requeue: {}", e);
                        return;
                    }
                };

                let queue_key = "jobs:queue:priority:0"; // Lower priority for retries
                if let Err(e) = app_state.redis.lpush(&queue_key, &job_data).await {
                    error!("Failed to requeue job: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Update message and job in database
    async fn update_message_and_job(
        app_state: &Arc<AppState>,
        message: &Message,
        job: &Job,
    ) -> Result<()> {
        let messages_collection = app_state.database.collection::<Message>("messages");
        let jobs_collection = app_state.database.collection::<Job>("jobs");

        // Update message
        messages_collection
            .replace_one(mongodb::bson::doc! {"id": &message.id}, message, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to update message: {}", e),
            })?;

        // Update job
        jobs_collection
            .replace_one(mongodb::bson::doc! {"id": &job.id}, job, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to update job: {}", e),
            })?;

        Ok(())
    }

    /// Cleanup expired jobs
    async fn cleanup_expired_jobs_loop(app_state: Arc<AppState>) {
        let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes

        loop {
            interval.tick().await;

            if let Err(e) = Self::cleanup_expired_jobs(&app_state).await {
                error!("Error during job cleanup: {}", e);
            }
        }
    }

    /// Remove expired jobs from the database
    async fn cleanup_expired_jobs(app_state: &Arc<AppState>) -> Result<()> {
        let jobs_collection = app_state.database.collection::<Job>("jobs");
        let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(24); // 24 hour timeout

        let result = jobs_collection
            .delete_many(
                mongodb::bson::doc! {
                    "created_at": {"$lt": cutoff_time},
                    "status": {"$in": ["Failed", "Expired"]},
                },
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to cleanup expired jobs: {}", e),
            })?;

        if result.deleted_count > 0 {
            info!("Cleaned up {} expired jobs", result.deleted_count);
        }

        Ok(())
    }
}

/// Extension trait for imports in other modules
use futures::stream::TryStreamExt;
