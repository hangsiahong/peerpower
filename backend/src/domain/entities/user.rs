use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::shared::types::{PhoneNumber, Carrier, ProviderStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub phone: PhoneNumber,
    pub did: Option<String>,
    pub evm_address: Option<String>,
    pub reputation_score: f64,
    pub is_provider: bool,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(phone: PhoneNumber) -> Self {
        let now = crate::shared::utils::now();
        Self {
            id: crate::shared::utils::generate_id(),
            phone,
            did: None,
            evm_address: None,
            reputation_score: 0.0,
            is_provider: false,
            is_verified: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn verify(&mut self) {
        self.is_verified = true;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn enable_provider(&mut self) {
        self.is_provider = true;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn update_reputation(&mut self, new_score: f64) {
        self.reputation_score = new_score.clamp(0.0, 100.0);
        self.updated_at = crate::shared::utils::now();
    }
}
