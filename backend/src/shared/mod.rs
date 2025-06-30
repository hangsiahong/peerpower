pub mod app_state;
pub mod errors;

pub use app_state::AppState;
pub use errors::{PeerPowerError, Result};

/// Common types used across the application
pub mod types {
    use serde::{Deserialize, Serialize};

    /// Phone number type for Cambodia
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PhoneNumber(String);

    impl PhoneNumber {
        pub fn new(phone: String) -> Result<Self, super::PeerPowerError> {
            // Basic validation for Cambodia phone numbers
            let cleaned = phone.trim().replace([' ', '-', '(', ')'], "");

            if cleaned.starts_with("+855") && cleaned.len() >= 12 {
                Ok(PhoneNumber(cleaned))
            } else if cleaned.starts_with("855") && cleaned.len() >= 11 {
                Ok(PhoneNumber(format!("+{}", cleaned)))
            } else if cleaned.starts_with("0") && cleaned.len() >= 9 {
                Ok(PhoneNumber(format!("+855{}", &cleaned[1..])))
            } else {
                Err(super::PeerPowerError::ValidationError {
                    field: "phone".to_string(),
                    message: "Invalid Cambodia phone number format".to_string(),
                })
            }
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    /// Carrier types in Cambodia
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Carrier {
        Smart,
        Metfone,
        Cellcard,
        Qb,
        Unknown,
    }

    impl Carrier {
        pub fn from_phone_number(phone: &PhoneNumber) -> Self {
            let phone_str = phone.as_str();

            // Smart prefixes
            if phone_str.starts_with("+85510")
                || phone_str.starts_with("+85515")
                || phone_str.starts_with("+85516")
                || phone_str.starts_with("+85593")
                || phone_str.starts_with("+85596")
            {
                return Carrier::Smart;
            }

            // Metfone prefixes
            if phone_str.starts_with("+85531")
                || phone_str.starts_with("+85560")
                || phone_str.starts_with("+85566")
                || phone_str.starts_with("+85567")
                || phone_str.starts_with("+85568")
            {
                return Carrier::Metfone;
            }

            // Cellcard prefixes
            if phone_str.starts_with("+85512")
                || phone_str.starts_with("+85561")
                || phone_str.starts_with("+85592")
                || phone_str.starts_with("+85595")
            {
                return Carrier::Cellcard;
            }

            // qb prefixes
            if phone_str.starts_with("+85513")
                || phone_str.starts_with("+85583")
                || phone_str.starts_with("+85584")
            {
                return Carrier::Qb;
            }

            Carrier::Unknown
        }
    }

    /// Message status throughout the system
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum MessageStatus {
        Pending,
        Assigned,
        Sent,
        Delivered,
        Failed,
        Cancelled,
    }

    /// Provider status
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ProviderStatus {
        Online,
        Offline,
        Busy,
        Suspended,
    }
}

/// Utilities for common operations
pub mod utils {
    use super::Result;
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    /// Generate a unique ID
    pub fn generate_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Get current UTC timestamp
    pub fn now() -> DateTime<Utc> {
        Utc::now()
    }
    /// Hash a password using Argon2
    pub fn hash_password(password: &str) -> Result<String> {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| super::PeerPowerError::Internal {
                message: format!("Password hashing failed: {}", e),
            })
    }

    /// Verify a password against its hash
    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Argon2,
        };

        let parsed_hash = PasswordHash::new(hash).map_err(|e| super::PeerPowerError::Internal {
            message: format!("Invalid password hash: {}", e),
        })?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }
}
