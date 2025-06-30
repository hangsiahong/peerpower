use crate::shared::types::PhoneNumber;
use crate::shared::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Authentication service for managing user sessions and tokens
#[async_trait]
pub trait AuthService: Send + Sync {
    async fn send_otp(&self, phone: &PhoneNumber) -> Result<String>;
    async fn verify_otp(&self, phone: &PhoneNumber, otp: &str) -> Result<AuthToken>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthToken>;
    async fn revoke_token(&self, token: &str) -> Result<()>;
    async fn validate_token(&self, token: &str) -> Result<TokenClaims>;
}

/// JWT token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user_id: String,
}

/// JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,       // user_id
    pub phone: String,     // phone number
    pub iat: i64,          // issued at
    pub exp: i64,          // expires at
    pub is_provider: bool, // provider status
}

/// OTP verification data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtpData {
    pub phone: PhoneNumber,
    pub code: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub attempts: u32,
}

impl OtpData {
    pub fn new(phone: PhoneNumber, code: String, ttl_minutes: i64) -> Self {
        let now = Utc::now();
        Self {
            phone,
            code,
            created_at: now,
            expires_at: now + chrono::Duration::minutes(ttl_minutes),
            attempts: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn is_valid(&self, code: &str) -> bool {
        !self.is_expired() && self.code == code && self.attempts < 3
    }

    pub fn increment_attempts(&mut self) {
        self.attempts += 1;
    }
}
