use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Path, Query, State},
    http::{request::Parts, StatusCode},
    response::Json,
    Json as JsonExtractor,
};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use validator::Validate;

use crate::domain::entities::message::MessagePriority;
use crate::domain::entities::{Job, Message};
use crate::domain::services::TokenClaims;
use crate::shared::types::PhoneNumber;
use crate::shared::{AppState, PeerPowerError, Result};

/// Extractor for authenticated user ID
pub struct AuthenticatedUser(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let claims = parts
            .extensions
            .get::<TokenClaims>()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AuthenticatedUser(claims.sub.clone()))
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct SendMessageRequest {
    #[validate(length(min = 10, max = 15, message = "Invalid recipient phone number"))]
    pub recipient: String,
    #[validate(length(
        min = 1,
        max = 500,
        message = "Message content must be 1-500 characters"
    ))]
    pub content: String,
    pub priority: Option<MessagePriority>,
    pub carrier_preference: Option<String>, // smart, metfone, cellcard
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub job_id: String,
    pub status: String,
    pub estimated_delivery_time: String,
    pub cost_estimate: f64, // In PPT tokens
}

#[derive(Debug, Serialize)]
pub struct MessageStatusResponse {
    pub message_id: String,
    pub job_id: String,
    pub status: String,
    pub provider_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub delivery_attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageListQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeliveryConfirmationRequest {
    pub status: String, // "delivered", "failed", "pending"
    pub delivery_time: Option<String>, // ISO 8601 timestamp
    pub error_message: Option<String>,
    pub provider_message_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeliveryConfirmationResponse {
    pub message_id: String,
    pub status: String,
    pub updated_at: String,
    pub provider_earnings: Option<f64>,
}

/// Submit SMS job for delivery
pub async fn send_message(
    State(app_state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    JsonExtractor(send_request): JsonExtractor<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>> {
    // Validate request
    send_request.validate()?;

    info!("Message send request from user: {}", user_id);

    // Parse recipient phone number
    let recipient = PhoneNumber::new(send_request.recipient)?;

    // Get priority early to avoid move issues
    let priority = send_request
        .priority
        .clone()
        .unwrap_or(MessagePriority::Normal);

    // Validate content (basic Khmer and Latin script support)
    if send_request.content.trim().is_empty() {
        return Err(PeerPowerError::ValidationError {
            field: "content".to_string(),
            message: "Message content cannot be empty".to_string(),
        });
    }

    // Create message ID (remove if not needed)

    // Create message entity using constructor
    let message = Message::new(
        user_id.clone(),
        send_request.content.clone(),
        recipient.clone(),
        priority,
        None, // client_reference
        None, // webhook_url
    );

    // For now, use a placeholder provider_id - in a real system this would be assigned by the job scheduler
    let placeholder_provider_id = "pending-assignment".to_string();

    // Create job entity using constructor
    let job = Job::new(message.id.clone(), placeholder_provider_id);

    // Store message and job in database
    let messages_collection = app_state.database.collection::<Message>("messages");
    let jobs_collection = app_state.database.collection::<Job>("jobs");

    messages_collection
        .insert_one(&message, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to store message: {}", e),
        })?;

    jobs_collection
        .insert_one(&job, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to store job: {}", e),
        })?;

    // TODO: Add job to Redis queue for processing
    let priority_score = message.get_priority_score();
    let queue_key = format!("jobs:queue:priority:{}", priority_score);
    let job_data = serde_json::to_string(&job).map_err(|e| PeerPowerError::Internal {
        message: format!("Failed to serialize job: {}", e),
    })?;

    app_state
        .redis
        .lpush(&queue_key, &job_data)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to queue job: {}", e),
        })?;

    // Calculate estimated delivery time and cost
    let estimated_delivery = chrono::Utc::now() + chrono::Duration::minutes(5);
    let cost_estimate = calculate_message_cost(&send_request.content, &message.priority);

    info!(
        "Message {} queued successfully for user {}",
        message.id, user_id
    );

    Ok(Json(SendMessageResponse {
        message_id: message.id,
        job_id: job.id,
        status: "queued".to_string(),
        estimated_delivery_time: estimated_delivery.to_rfc3339(),
        cost_estimate,
    }))
}

/// Get message status
pub async fn get_message_status(
    State(app_state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<MessageStatusResponse>> {
    // Find message
    let messages_collection = app_state.database.collection::<Message>("messages");
    let message = messages_collection
        .find_one(
            mongodb::bson::doc! {
                "id": &message_id,
                "client_id": &user_id
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch message: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Message with ID: {}", message_id),
        })?;

    // Find associated job
    let jobs_collection = app_state.database.collection::<Job>("jobs");
    let job = jobs_collection
        .find_one(mongodb::bson::doc! {"message_id": &message_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch job: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Job for message: {}", message_id),
        })?;

    Ok(Json(MessageStatusResponse {
        message_id: message.id,
        job_id: job.id,
        status: format!("{:?}", message.status).to_lowercase(),
        provider_id: message.provider_id,
        created_at: message.created_at.to_rfc3339(),
        updated_at: message.updated_at.to_rfc3339(),
        delivery_attempts: job.retry_count,
        last_error: job.error_message,
    }))
}

/// List user's messages with pagination
pub async fn list_messages(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<MessageListQuery>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<Vec<MessageStatusResponse>>> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let skip = (page - 1) * limit;

    // Build query filter
    let mut filter = mongodb::bson::doc! {"client_id": &user_id};
    if let Some(status) = params.status {
        filter.insert("status", status);
    }

    // Create find options with pagination and sorting
    let mut find_options = mongodb::options::FindOptions::default();
    find_options.skip = Some(skip as u64);
    find_options.limit = Some(limit as i64);
    find_options.sort = Some(mongodb::bson::doc! {"created_at": -1}); // Most recent first

    // Find messages with pagination
    let messages_collection = app_state.database.collection::<Message>("messages");
    let mut cursor = messages_collection
        .find(filter, find_options)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch messages: {}", e),
        })?;

    let mut messages = Vec::new();
    while let Ok(Some(message)) = cursor.try_next().await {
        // Find associated job
        let jobs_collection = app_state.database.collection::<Job>("jobs");
        if let Ok(Some(job)) = jobs_collection
            .find_one(mongodb::bson::doc! {"message_id": &message.id}, None)
            .await
        {
            messages.push(MessageStatusResponse {
                message_id: message.id,
                job_id: job.id,
                status: format!("{:?}", message.status).to_lowercase(),
                provider_id: message.provider_id,
                created_at: message.created_at.to_rfc3339(),
                updated_at: message.updated_at.to_rfc3339(),
                delivery_attempts: job.retry_count,
                last_error: job.error_message,
            });
        }
    }

    Ok(Json(messages))
}

/// Confirm message delivery (called by providers)
pub async fn confirm_delivery(
    State(app_state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    JsonExtractor(delivery_request): JsonExtractor<DeliveryConfirmationRequest>,
) -> Result<Json<DeliveryConfirmationResponse>> {
    delivery_request.validate()?;

    info!("Delivery confirmation for message {} from user {}", message_id, user_id);

    // Find the message and verify the user is the assigned provider
    let messages_collection = app_state.database.collection::<Message>("messages");
    let mut message = messages_collection
        .find_one(mongodb::bson::doc! {"id": &message_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch message: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Message with ID: {}", message_id),
        })?;

    // Verify the user is the assigned provider for this message
    let providers_collection = app_state.database.collection::<crate::domain::entities::Provider>("providers");
    let provider = providers_collection
        .find_one(mongodb::bson::doc! {"user_id": &user_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch provider: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Provider for user: {}", user_id),
        })?;

    // Check if this provider is assigned to this message
    if message.provider_id.as_ref() != Some(&provider.id) {
        return Err(PeerPowerError::ValidationError {
            field: "provider".to_string(),
            message: "You are not assigned to this message".to_string(),
        });
    }

    // Update message status based on delivery confirmation
    let new_status = match delivery_request.status.as_str() {
        "delivered" => crate::shared::types::MessageStatus::Delivered,
        "failed" => crate::shared::types::MessageStatus::Failed,
        "pending" => crate::shared::types::MessageStatus::Sent,
        _ => return Err(PeerPowerError::ValidationError {
            field: "status".to_string(),
            message: "Invalid delivery status".to_string(),
        }),
    };

    message.status = new_status;
    message.updated_at = chrono::Utc::now();

    // Update the message in database
    messages_collection
        .update_one(
            mongodb::bson::doc! {"id": &message_id},
            mongodb::bson::doc! {
                "$set": {
                    "status": format!("{:?}", message.status),
                    "updated_at": message.updated_at
                }
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to update message: {}", e),
        })?;

    // Update the associated job
    let jobs_collection = app_state.database.collection::<Job>("jobs");
    let job_update = if delivery_request.status == "delivered" {
        mongodb::bson::doc! {
            "$set": {
                "status": "completed",
                "completed_at": chrono::Utc::now(),
                "updated_at": chrono::Utc::now()
            }
        }
    } else {
        mongodb::bson::doc! {
            "$set": {
                "status": "failed",
                "error_message": delivery_request.error_message.unwrap_or("Delivery failed".to_string()),
                "updated_at": chrono::Utc::now()
            }
        }
    };

    jobs_collection
        .update_one(
            mongodb::bson::doc! {"message_id": &message_id},
            job_update,
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to update job: {}", e),
        })?;

    // Calculate provider earnings if delivered
    let provider_earnings = if delivery_request.status == "delivered" {
        Some(calculate_provider_earnings(&message.content, &message.priority))
    } else {
        None
    };

    // If delivered, update provider stats and earnings
    if delivery_request.status == "delivered" {
        let earnings = provider_earnings.unwrap_or(0.0);
        providers_collection
            .update_one(
                mongodb::bson::doc! {"id": &provider.id},
                mongodb::bson::doc! {
                    "$inc": {
                        "total_messages_delivered": 1,
                        "earnings_total": earnings
                    },
                    "$set": {
                        "updated_at": chrono::Utc::now()
                    }
                },
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to update provider stats: {}", e),
            })?;
    }

    info!("Message {} delivery confirmed with status: {}", message_id, delivery_request.status);

    Ok(Json(DeliveryConfirmationResponse {
        message_id: message.id,
        status: format!("{:?}", message.status).to_lowercase(),
        updated_at: message.updated_at.to_rfc3339(),
        provider_earnings,
    }))
}

/// Webhook endpoint for external delivery confirmations
pub async fn delivery_webhook(
    State(app_state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
    JsonExtractor(delivery_request): JsonExtractor<DeliveryConfirmationRequest>,
) -> Result<Json<serde_json::Value>> {
    delivery_request.validate()?;

    info!("Webhook delivery confirmation for message {}", message_id);

    // Find the message
    let messages_collection = app_state.database.collection::<Message>("messages");
    let mut message = messages_collection
        .find_one(mongodb::bson::doc! {"id": &message_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch message: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Message with ID: {}", message_id),
        })?;

    // Update message status
    let new_status = match delivery_request.status.as_str() {
        "delivered" => crate::shared::types::MessageStatus::Delivered,
        "failed" => crate::shared::types::MessageStatus::Failed,
        "pending" => crate::shared::types::MessageStatus::Sent,
        _ => return Err(PeerPowerError::ValidationError {
            field: "status".to_string(),
            message: "Invalid delivery status".to_string(),
        }),
    };

    message.status = new_status;
    message.updated_at = chrono::Utc::now();

    // Update the message in database
    messages_collection
        .update_one(
            mongodb::bson::doc! {"id": &message_id},
            mongodb::bson::doc! {
                "$set": {
                    "status": format!("{:?}", message.status),
                    "updated_at": message.updated_at
                }
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to update message: {}", e),
        })?;

    info!("Webhook processed successfully for message {}", message_id);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message_id": message_id,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Calculate message cost based on content length and priority
fn calculate_message_cost(content: &str, priority: &MessagePriority) -> f64 {
    let base_cost = 0.01; // Base cost in PPT tokens
    let length_multiplier = (content.len() as f64 / 160.0).ceil(); // SMS is typically 160 chars

    let priority_multiplier = match priority {
        MessagePriority::Low => 0.8,
        MessagePriority::Normal => 1.0,
        MessagePriority::High => 1.5,
        MessagePriority::Urgent => 2.0,
    };

    base_cost * length_multiplier * priority_multiplier
}

/// Calculate provider earnings for a delivered message
fn calculate_provider_earnings(content: &str, priority: &MessagePriority) -> f64 {
    let base_earnings = 0.008; // 80% of base cost goes to provider
    let length_multiplier = (content.len() as f64 / 160.0).ceil();

    let priority_multiplier = match priority {
        MessagePriority::Low => 0.8,
        MessagePriority::Normal => 1.0,
        MessagePriority::High => 1.5,
        MessagePriority::Urgent => 2.0,
    };

    base_earnings * length_multiplier * priority_multiplier
}
