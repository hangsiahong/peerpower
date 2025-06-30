use async_trait::async_trait;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::AuthConfig;
use crate::domain::entities::User;
use crate::domain::repositories::UserRepository;
use crate::domain::services::{AuthService, AuthToken, OtpData, TokenClaims};
use crate::infrastructure::database::RedisConnection;
use crate::shared::types::PhoneNumber;
use crate::shared::{PeerPowerError, Result};

pub struct AuthServiceImpl {
    config: AuthConfig,
    redis: Arc<RedisConnection>,
    user_repo: Arc<dyn UserRepository>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl AuthServiceImpl {
    pub fn new(
        config: AuthConfig,
        redis: Arc<RedisConnection>,
        user_repo: Arc<dyn UserRepository>,
    ) -> Self {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());

        Self {
            config,
            redis,
            user_repo,
            encoding_key,
            decoding_key,
        }
    }

    fn generate_otp(&self) -> String {
        // For development/testing, use a fixed OTP
        if cfg!(debug_assertions) {
            "123456".to_string()
        } else {
            let mut rng = rand::thread_rng();
            format!("{:06}", rng.gen_range(100000..999999))
        }
    }

    fn otp_key(&self, phone: &PhoneNumber) -> String {
        format!("otp:{}", phone.as_str())
    }

    fn rate_limit_key(&self, phone: &PhoneNumber) -> String {
        format!("rate_limit:otp:{}", phone.as_str())
    }

    async fn check_rate_limit(&self, phone: &PhoneNumber) -> Result<()> {
        let key = self.rate_limit_key(phone);
        let current_count = self.redis.increment(&key).await?;

        if current_count == 1 {
            // Set expiration for the first request (1 hour window)
            self.redis.set(&key, "1", Some(3600)).await?;
        }

        if current_count > 5 {
            return Err(PeerPowerError::RateLimitExceeded {
                resource: "OTP requests".to_string(),
            });
        }

        Ok(())
    }

    async fn store_otp(&self, otp_data: &OtpData) -> Result<()> {
        let key = self.otp_key(&otp_data.phone);
        let value = serde_json::to_string(otp_data).map_err(|e| PeerPowerError::Internal {
            message: format!("Failed to serialize OTP data: {}", e),
        })?;

        self.redis
            .set(
                &key,
                &value,
                Some(self.config.otp_expiration_minutes as usize * 60),
            )
            .await?;

        Ok(())
    }

    async fn get_otp(&self, phone: &PhoneNumber) -> Result<Option<OtpData>> {
        let key = self.otp_key(phone);
        let value = self.redis.get(&key).await?;

        match value {
            Some(data) => {
                let otp_data: OtpData =
                    serde_json::from_str(&data).map_err(|e| PeerPowerError::Internal {
                        message: format!("Failed to deserialize OTP data: {}", e),
                    })?;
                Ok(Some(otp_data))
            }
            None => Ok(None),
        }
    }

    async fn delete_otp(&self, phone: &PhoneNumber) -> Result<()> {
        let key = self.otp_key(phone);
        self.redis.delete(&key).await?;
        Ok(())
    }

    async fn generate_tokens(&self, user: &User) -> Result<AuthToken> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.config.jwt_expiration_hours);

        let claims = TokenClaims {
            sub: user.id.clone(),
            phone: user.phone.as_str().to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            is_provider: user.is_provider,
        };

        let access_token =
            encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
                PeerPowerError::Internal {
                    message: format!("Failed to generate access token: {}", e),
                }
            })?;

        // Generate refresh token (simple UUID for now)
        let refresh_token = crate::shared::utils::generate_id();

        // Store refresh token in Redis (valid for 30 days)
        let refresh_key = format!("refresh_token:{}", refresh_token);
        self.redis
            .set(&refresh_key, &user.id, Some(30 * 24 * 3600))
            .await?;

        Ok(AuthToken {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.jwt_expiration_hours * 3600,
            user_id: user.id.clone(),
        })
    }
}

#[async_trait]
impl AuthService for AuthServiceImpl {
    async fn send_otp(&self, phone: &PhoneNumber) -> Result<String> {
        info!("Sending OTP to phone: {}", phone.as_str());

        // Check rate limiting
        self.check_rate_limit(phone).await?;

        // Generate OTP
        let otp_code = self.generate_otp();
        let otp_data = OtpData::new(
            phone.clone(),
            otp_code.clone(),
            self.config.otp_expiration_minutes,
        );

        // Store OTP in Redis
        self.store_otp(&otp_data).await?;

        // TODO: Send actual SMS via provider
        // For now, just log it (in production, integrate with SMS service)
        info!("OTP for {}: {}", phone.as_str(), otp_code);

        Ok("OTP sent successfully".to_string())
    }

    async fn verify_otp(&self, phone: &PhoneNumber, otp: &str) -> Result<AuthToken> {
        info!("Verifying OTP for phone: {}", phone.as_str());

        // Get stored OTP
        let mut otp_data =
            self.get_otp(phone)
                .await?
                .ok_or_else(|| PeerPowerError::AuthenticationFailed {
                    reason: "OTP not found or expired".to_string(),
                })?;

        // Increment attempt count
        otp_data.increment_attempts();
        self.store_otp(&otp_data).await?;

        // Validate OTP
        if !otp_data.is_valid(otp) {
            return Err(PeerPowerError::AuthenticationFailed {
                reason: if otp_data.is_expired() {
                    "OTP has expired".to_string()
                } else if otp_data.attempts >= 3 {
                    "Too many invalid attempts".to_string()
                } else {
                    "Invalid OTP".to_string()
                },
            });
        }

        // Clean up used OTP
        self.delete_otp(phone).await?;

        // Find or create user
        info!("Looking up user by phone: {}", phone.as_str());
        let user = match self.user_repo.find_by_phone(phone).await? {
            Some(user) => {
                info!("Found existing user: {}", user.id);
                // Update user verification status
                let mut updated_user = user;
                if !updated_user.is_verified {
                    updated_user.verify();
                    self.user_repo.update(&updated_user).await?;
                    info!("Updated user verification status for: {}", updated_user.id);
                }
                updated_user
            }
            None => {
                info!("User not found, creating new user");
                // Create new user
                let mut new_user = User::new(phone.clone());
                new_user.verify();
                info!("Created new user object with ID: {}", new_user.id);

                match self.user_repo.create(&new_user).await {
                    Ok(()) => {
                        info!("Successfully saved user to database: {}", new_user.id);
                    }
                    Err(e) => {
                        warn!("Failed to save user to database: {:?}", e);
                        return Err(e);
                    }
                }

                // Fetch the newly created user from the database to ensure we have the complete object
                info!("Fetching newly created user from database");
                match self.user_repo.find_by_phone(phone).await? {
                    Some(user) => {
                        info!("Successfully retrieved newly created user: {}", user.id);
                        user
                    }
                    None => {
                        warn!("Failed to retrieve newly created user from database");
                        return Err(PeerPowerError::Internal {
                            message: "Failed to retrieve newly created user".to_string(),
                        });
                    }
                }
            }
        };

        // Generate tokens
        let tokens = self.generate_tokens(&user).await?;

        info!("Successfully authenticated user: {}", user.id);
        Ok(tokens)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthToken> {
        let refresh_key = format!("refresh_token:{}", refresh_token);
        let user_id = self.redis.get(&refresh_key).await?.ok_or_else(|| {
            PeerPowerError::AuthenticationFailed {
                reason: "Invalid refresh token".to_string(),
            }
        })?;

        // Get user
        let user = self.user_repo.find_by_id(&user_id).await?.ok_or_else(|| {
            PeerPowerError::AuthenticationFailed {
                reason: "User not found".to_string(),
            }
        })?;

        // Generate new tokens
        self.generate_tokens(&user).await
    }

    async fn revoke_token(&self, token: &str) -> Result<()> {
        // For JWT tokens, we'd typically add them to a blacklist
        // For simplicity, we'll just delete the refresh token if provided
        let refresh_key = format!("refresh_token:{}", token);
        self.redis.delete(&refresh_key).await?;
        Ok(())
    }

    async fn validate_token(&self, token: &str) -> Result<TokenClaims> {
        let validation = Validation::new(Algorithm::HS256);

        let token_data =
            decode::<TokenClaims>(token, &self.decoding_key, &validation).map_err(|e| {
                PeerPowerError::AuthenticationFailed {
                    reason: format!("Invalid token: {}", e),
                }
            })?;

        Ok(token_data.claims)
    }
}
