use axum::{extract::State, http::StatusCode, response::Json, Json as JsonExtractor};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use validator::Validate;

use crate::domain::services::AuthService;
use crate::shared::types::PhoneNumber;
use crate::shared::{AppState, PeerPowerError, Result};

#[derive(Debug, Deserialize, Validate)]
pub struct SendOtpRequest {
    #[validate(length(min = 10, max = 15, message = "Invalid phone number length"))]
    pub phone: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyOtpRequest {
    #[validate(length(min = 10, max = 15, message = "Invalid phone number length"))]
    pub phone: String,
    #[validate(length(equal = 6, message = "OTP must be 6 digits"))]
    pub otp: String,
}

#[derive(Debug, Serialize)]
pub struct SendOtpResponse {
    pub message: String,
    pub expires_in_minutes: i64,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub phone: String,
    pub is_provider: bool,
    pub is_verified: bool,
}

/// Send OTP to phone number
pub async fn send_otp(
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<SendOtpRequest>,
) -> Result<Json<SendOtpResponse>> {
    // Validate request
    request.validate()?;

    // Parse phone number
    let phone = PhoneNumber::new(request.phone)?;

    info!("OTP request for phone: {}", phone.as_str());

    // Send OTP
    app_state.auth_service.send_otp(&phone).await?;

    Ok(Json(SendOtpResponse {
        message: "OTP sent successfully".to_string(),
        expires_in_minutes: 5, // TODO: Get from config
    }))
}

/// Verify OTP and authenticate user
pub async fn verify_otp(
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<VerifyOtpRequest>,
) -> Result<Json<AuthResponse>> {
    // Validate request
    request.validate()?;

    // Parse phone number
    let phone = PhoneNumber::new(request.phone)?;

    info!("OTP verification for phone: {}", phone.as_str());

    // Verify OTP and get tokens
    let auth_token = app_state
        .auth_service
        .verify_otp(&phone, &request.otp)
        .await?;

    // TODO: Get user info from token claims or user repository
    let user_info = UserInfo {
        id: auth_token.user_id.clone(),
        phone: phone.as_str().to_string(),
        is_provider: false, // TODO: Get from user
        is_verified: true,
    };

    Ok(Json(AuthResponse {
        access_token: auth_token.access_token,
        refresh_token: auth_token.refresh_token,
        token_type: auth_token.token_type,
        expires_in: auth_token.expires_in,
        user: user_info,
    }))
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Refresh access token
pub async fn refresh_token(
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<RefreshTokenRequest>,
) -> Result<Json<AuthResponse>> {
    info!("Token refresh request");

    // Refresh token
    let auth_token = app_state
        .auth_service
        .refresh_token(&request.refresh_token)
        .await?;

    // TODO: Get user info
    let user_info = UserInfo {
        id: auth_token.user_id.clone(),
        phone: "".to_string(), // TODO: Get from user
        is_provider: false,
        is_verified: true,
    };

    Ok(Json(AuthResponse {
        access_token: auth_token.access_token,
        refresh_token: auth_token.refresh_token,
        token_type: auth_token.token_type,
        expires_in: auth_token.expires_in,
        user: user_info,
    }))
}

/// Logout user (revoke tokens)
pub async fn logout(
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<RefreshTokenRequest>,
) -> Result<StatusCode> {
    info!("Logout request");

    app_state
        .auth_service
        .revoke_token(&request.refresh_token)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
