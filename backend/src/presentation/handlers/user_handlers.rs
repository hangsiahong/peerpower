use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Path, Request, State},
    http::{request::Parts, StatusCode},
    response::Json,
    Json as JsonExtractor,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use validator::Validate;

use crate::domain::entities::User;
use crate::domain::services::TokenClaims;
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

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub phone: String,
    pub did: Option<String>,
    pub evm_address: Option<String>,
    pub reputation_score: f64,
    pub is_provider: bool,
    pub is_verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&User> for UserProfileResponse {
    fn from(user: &User) -> Self {
        Self {
            id: user.id.clone(),
            phone: user.phone.as_str().to_string(),
            did: user.did.clone(),
            evm_address: user.evm_address.clone(),
            reputation_score: user.reputation_score,
            is_provider: user.is_provider,
            is_verified: user.is_verified,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, message = "DID cannot be empty"))]
    pub did: Option<String>,
    #[validate(length(equal = 42, message = "Invalid EVM address format"))]
    pub evm_address: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterProviderRequest {
    #[validate(length(min = 1, message = "Carrier is required"))]
    pub carrier: String,
    pub location: Option<ProviderLocation>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProviderLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
}

/// Get user profile (protected route)
pub async fn get_user_profile(
    AuthenticatedUser(user_id): AuthenticatedUser,
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<UserProfileResponse>> {
    info!("Getting profile for user: {}", user_id);

    // Get user from repository
    let user = app_state
        .user_repository
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("User with ID: {}", user_id),
        })?;

    Ok(Json(UserProfileResponse::from(&user)))
}

/// Update user profile (protected route)
pub async fn update_user_profile(
    AuthenticatedUser(user_id): AuthenticatedUser,
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(update_request): JsonExtractor<UpdateProfileRequest>,
) -> Result<Json<UserProfileResponse>> {
    // Validate request
    update_request.validate()?;

    info!("Updating profile for user: {}", user_id);

    // Get existing user
    let mut user = app_state
        .user_repository
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("User with ID: {}", user_id),
        })?;

    // Update fields
    if let Some(did) = update_request.did {
        user.did = Some(did);
    }

    if let Some(evm_address) = update_request.evm_address {
        // Validate EVM address format (basic check)
        if !evm_address.starts_with("0x") || evm_address.len() != 42 {
            return Err(PeerPowerError::ValidationError {
                field: "evm_address".to_string(),
                message: "Invalid EVM address format".to_string(),
            });
        }
        user.evm_address = Some(evm_address);
    }

    // Update timestamp
    user.updated_at = chrono::Utc::now();

    // Save updated user
    app_state.user_repository.update(&user).await?;

    info!("Successfully updated profile for user: {}", user_id);
    Ok(Json(UserProfileResponse::from(&user)))
}

/// Register as a provider (protected route)
pub async fn register_provider(
    AuthenticatedUser(user_id): AuthenticatedUser,
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(register_request): JsonExtractor<RegisterProviderRequest>,
) -> Result<StatusCode> {
    // Validate request
    register_request.validate()?;

    info!("Registering provider for user: {}", user_id);

    // Validate carrier
    let valid_carriers = ["smart", "metfone", "cellcard"];
    if !valid_carriers.contains(&register_request.carrier.to_lowercase().as_str()) {
        return Err(PeerPowerError::ValidationError {
            field: "carrier".to_string(),
            message: "Invalid carrier. Must be one of: Smart, Metfone, Cellcard".to_string(),
        });
    }

    // Update user to be a provider
    let collection = app_state.database.collection::<User>("users");
    let mut update_doc = mongodb::bson::doc! {
        "is_provider": true,
        "updated_at": chrono::Utc::now()
    };

    // Store provider-specific data (for now in the same collection)
    // TODO: Move to separate providers collection
    update_doc.insert("provider_carrier", register_request.carrier.to_lowercase());

    if let Some(location) = register_request.location {
        update_doc.insert(
            "provider_location",
            mongodb::bson::to_bson(&location).map_err(|e| PeerPowerError::Internal {
                message: format!("Failed to serialize location: {}", e),
            })?,
        );
    }

    let result = collection
        .update_one(
            mongodb::bson::doc! {"id": &user_id},
            mongodb::bson::doc! {"$set": update_doc},
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to register provider: {}", e),
        })?;

    if result.matched_count == 0 {
        return Err(PeerPowerError::NotFound {
            resource: format!("User with ID: {}", user_id),
        });
    }

    info!("Successfully registered provider for user: {}", user_id);
    Ok(StatusCode::CREATED)
}
