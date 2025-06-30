use axum::{
    extract::{Path, Query, State},
    response::Json,
    Json as JsonExtractor,
};
use chrono::{DateTime, Utc};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use validator::Validate;

use crate::domain::entities::provider::{Location, Provider};
use crate::presentation::handlers::message_handlers::AuthenticatedUser;
use crate::shared::types::{Carrier, PhoneNumber, ProviderStatus};
use crate::shared::{AppState, PeerPowerError, Result};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterProviderRequest {
    #[validate(length(min = 10, max = 15, message = "Invalid phone number"))]
    pub phone: String,
    pub fcm_token: String, // For push notifications
    pub location: Option<Location>,
}

#[derive(Debug, Serialize)]
pub struct RegisterProviderResponse {
    pub provider_id: String,
    pub status: String,
    pub registered_at: String,
    pub carrier: String,
    pub phone: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatusResponse {
    pub provider_id: String,
    pub user_id: String,
    pub phone: String,
    pub carrier: String,
    pub status: String,
    pub location: Option<Location>,
    pub last_heartbeat: Option<String>,
    pub message_count_today: u32,
    pub success_rate: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ProviderListQuery {
    pub status: Option<String>,
    pub carrier: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub status: ProviderStatus,
    pub location: Option<String>,
    pub battery_level: Option<u8>,
    pub signal_strength: Option<u8>,
}

/// Register a new SMS provider
pub async fn register_provider(
    State(app_state): State<Arc<AppState>>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    JsonExtractor(register_request): JsonExtractor<RegisterProviderRequest>,
) -> Result<Json<RegisterProviderResponse>> {
    // Validate request
    register_request.validate()?;

    info!("Provider registration request from user: {}", user_id);

    // Parse and validate phone number
    let phone = PhoneNumber::new(register_request.phone)?;
    let carrier = Carrier::from_phone_number(&phone);

    // Check if user exists and is verified
    let user = app_state
        .user_repository
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("User with ID: {}", user_id),
        })?;

    if !user.is_verified {
        return Err(PeerPowerError::ValidationError {
            field: "user".to_string(),
            message: "User must be verified to register as provider".to_string(),
        });
    }

    // Check if provider with this phone already exists
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let existing_provider = providers_collection
        .find_one(
            mongodb::bson::doc! {
                "phone": phone.as_str(),
                "user_id": &user_id
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to check existing provider: {}", e),
        })?;

    if existing_provider.is_some() {
        return Err(PeerPowerError::ValidationError {
            field: "phone".to_string(),
            message: "Provider with this phone number already registered".to_string(),
        });
    }

    // Create new provider using the correct constructor
    let mut provider = Provider::new(user_id.clone(), phone.clone(), carrier.clone());

    // Set additional fields
    provider.fcm_token = Some(register_request.fcm_token);
    provider.location = register_request.location;

    // Store provider in database
    providers_collection
        .insert_one(&provider, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to store provider: {}", e),
        })?;

    // Add provider to Redis active providers set for quick lookup
    let redis_key = format!("providers:active:{:?}", carrier);
    app_state
        .redis
        .sadd(&redis_key, &provider.id)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to cache provider: {}", e),
        })?;

    info!(
        "Provider {} registered successfully for user {}",
        provider.id, user_id
    );

    Ok(Json(RegisterProviderResponse {
        provider_id: provider.id,
        status: format!("{:?}", provider.status).to_lowercase(),
        registered_at: provider.created_at.to_rfc3339(),
        carrier: format!("{:?}", provider.carrier),
        phone: provider.phone.as_str().to_string(),
    }))
}

/// Get provider status and details
pub async fn get_provider_status(
    State(app_state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<ProviderStatusResponse>> {
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let provider = providers_collection
        .find_one(
            mongodb::bson::doc! {
                "id": &provider_id,
                "user_id": &user_id
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch provider: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Provider with ID: {}", provider_id),
        })?;

    // Calculate today's message count and success rate
    let messages_collection = app_state
        .database
        .collection::<crate::domain::entities::message::Message>("messages");
    let today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let message_count_today = messages_collection
        .count_documents(
            mongodb::bson::doc! {
                "provider_id": &provider.id,
                "created_at": {"$gte": today_start}
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count messages: {}", e),
        })? as u32;

    // TODO: Calculate actual success rate from delivery confirmations
    let success_rate = 95.0; // Placeholder

    Ok(Json(ProviderStatusResponse {
        provider_id: provider.id,
        user_id: provider.user_id,
        phone: provider.phone.as_str().to_string(),
        carrier: format!("{:?}", provider.carrier),
        status: format!("{:?}", provider.status).to_lowercase(),
        location: provider.location,
        last_heartbeat: provider.last_heartbeat.map(|dt| dt.to_rfc3339()),
        message_count_today,
        success_rate,
        created_at: provider.created_at.to_rfc3339(),
        updated_at: provider.updated_at.to_rfc3339(),
    }))
}

/// List user's providers
pub async fn list_providers(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ProviderListQuery>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<Vec<ProviderStatusResponse>>> {
    use mongodb::bson::{doc, Document};

    let providers_collection = app_state.database.collection::<Document>("providers");

    // Build query filter
    let mut filter = doc! {"user_id": &user_id};
    if let Some(status) = params.status {
        filter.insert("status", status);
    }
    if let Some(carrier) = params.carrier {
        filter.insert("carrier", carrier);
    }

    // Find providers
    let mut cursor =
        providers_collection
            .find(filter, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to query providers: {}", e),
            })?;

    let mut providers = Vec::new();
    while let Some(doc) = cursor
        .try_next()
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to iterate providers: {}", e),
        })?
    {
        // Extract fields from the document
        let provider_id = doc.get_str("id").unwrap_or("").to_string();
        let user_id = doc.get_str("user_id").unwrap_or("").to_string();
        let phone = doc.get_str("phone").unwrap_or("").to_string();
        let carrier = doc.get_str("carrier").unwrap_or("Unknown").to_string();
        let status = doc.get_str("status").unwrap_or("offline").to_string();

        // Handle datetime fields safely
        let created_at = doc
            .get_datetime("created_at")
            .map(|dt| DateTime::<Utc>::from(*dt).to_rfc3339())
            .unwrap_or_else(|_| Utc::now().to_rfc3339());

        let updated_at = doc
            .get_datetime("updated_at")
            .map(|dt| DateTime::<Utc>::from(*dt).to_rfc3339())
            .unwrap_or_else(|_| Utc::now().to_rfc3339());

        let last_heartbeat = doc
            .get_datetime("last_heartbeat")
            .map(|dt| DateTime::<Utc>::from(*dt).to_rfc3339())
            .ok();

        // For now, use placeholder values for message count and success rate
        // TODO: Calculate actual values from message history
        providers.push(ProviderStatusResponse {
            provider_id,
            user_id,
            phone,
            carrier,
            status,
            location: None, // TODO: Handle location properly
            last_heartbeat,
            message_count_today: 0, // TODO: Calculate actual count
            success_rate: 95.0,     // TODO: Calculate actual success rate
            created_at,
            updated_at,
        });
    }

    Ok(Json(providers))
}

/// Provider heartbeat endpoint (keeps provider status updated)
pub async fn provider_heartbeat(
    State(app_state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    JsonExtractor(heartbeat_request): JsonExtractor<HeartbeatRequest>,
) -> Result<Json<serde_json::Value>> {
    let providers_collection = app_state.database.collection::<Provider>("providers");

    // Update provider status and last heartbeat
    let mut update_doc = mongodb::bson::doc! {
        "$set": {
            "status": format!("{:?}", heartbeat_request.status),
            "last_heartbeat": chrono::Utc::now(),
            "updated_at": chrono::Utc::now(),
        }
    };

    // Add optional fields if present
    if let Some(location) = heartbeat_request.location {
        update_doc
            .get_mut("$set")
            .unwrap()
            .as_document_mut()
            .unwrap()
            .insert("location", location);
    }
    if let Some(battery_level) = heartbeat_request.battery_level {
        update_doc
            .get_mut("$set")
            .unwrap()
            .as_document_mut()
            .unwrap()
            .insert("battery_level", battery_level as i32);
    }
    if let Some(signal_strength) = heartbeat_request.signal_strength {
        update_doc
            .get_mut("$set")
            .unwrap()
            .as_document_mut()
            .unwrap()
            .insert("signal_strength", signal_strength as i32);
    }

    let result = providers_collection
        .update_one(
            mongodb::bson::doc! {
                "id": &provider_id,
                "user_id": &user_id
            },
            update_doc,
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to update provider: {}", e),
        })?;

    if result.matched_count == 0 {
        return Err(PeerPowerError::NotFound {
            resource: format!("Provider with ID: {}", provider_id),
        });
    }

    // Update Redis cache if provider is online
    if heartbeat_request.status == ProviderStatus::Online {
        // Add to active providers set
        let redis_key = "providers:active"; // We'll determine SIM type from database if needed
        app_state
            .redis
            .sadd(&redis_key, &provider_id)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to cache provider: {}", e),
            })?;
    }

    info!(
        "Heartbeat received from provider {} (user: {})",
        provider_id, user_id
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Update provider status (admin function or automatic)
pub async fn update_provider_status(
    State(app_state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    AuthenticatedUser(user_id): AuthenticatedUser,
    JsonExtractor(status_request): JsonExtractor<serde_json::Value>,
) -> Result<Json<serde_json::Value>> {
    let status_str =
        status_request["status"]
            .as_str()
            .ok_or_else(|| PeerPowerError::ValidationError {
                field: "status".to_string(),
                message: "Status field is required".to_string(),
            })?;

    let status = match status_str.to_lowercase().as_str() {
        "online" => ProviderStatus::Online,
        "offline" => ProviderStatus::Offline,
        "busy" => ProviderStatus::Busy,
        "suspended" => ProviderStatus::Suspended,
        _ => {
            return Err(PeerPowerError::ValidationError {
                field: "status".to_string(),
                message: "Invalid status value. Must be one of: online, offline, busy, suspended"
                    .to_string(),
            })
        }
    };

    let providers_collection = app_state.database.collection::<Provider>("providers");
    let result = providers_collection
        .update_one(
            mongodb::bson::doc! {
                "id": &provider_id,
                "user_id": &user_id
            },
            mongodb::bson::doc! {
                "$set": {
                    "status": format!("{:?}", status),
                    "updated_at": chrono::Utc::now(),
                }
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to update provider status: {}", e),
        })?;

    if result.matched_count == 0 {
        return Err(PeerPowerError::NotFound {
            resource: format!("Provider with ID: {}", provider_id),
        });
    }

    Ok(Json(serde_json::json!({
        "status": "updated",
        "provider_id": provider_id,
        "new_status": format!("{:?}", status).to_lowercase(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
