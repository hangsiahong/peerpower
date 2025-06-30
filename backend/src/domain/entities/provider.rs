use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::shared::types::{PhoneNumber, Carrier, ProviderStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub user_id: String,
    pub phone: PhoneNumber,
    pub carrier: Carrier,
    pub status: ProviderStatus,
    pub fcm_token: Option<String>,
    pub location: Option<Location>,
    pub current_load: u32,
    pub max_daily_messages: u32,
    pub messages_sent_today: u32,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub reputation_score: f64,
    pub success_rate: f64,
    pub total_messages_sent: u64,
    pub total_messages_delivered: u64,
    pub earnings_total: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub city: Option<String>,
    pub province: Option<String>,
}

impl Provider {
    pub fn new(user_id: String, phone: PhoneNumber, carrier: Carrier) -> Self {
        let now = crate::shared::utils::now();
        Self {
            id: crate::shared::utils::generate_id(),
            user_id,
            phone,
            carrier,
            status: ProviderStatus::Offline,
            fcm_token: None,
            location: None,
            current_load: 0,
            max_daily_messages: 50, // Default limit
            messages_sent_today: 0,
            last_heartbeat: None,
            reputation_score: 50.0, // Start with neutral score
            success_rate: 100.0, // Start optimistic
            total_messages_sent: 0,
            total_messages_delivered: 0,
            earnings_total: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self.status, ProviderStatus::Online) 
            && self.current_load < 5 // Max concurrent messages
            && self.messages_sent_today < self.max_daily_messages
            && self.is_heartbeat_recent()
    }

    pub fn is_heartbeat_recent(&self) -> bool {
        if let Some(last_heartbeat) = self.last_heartbeat {
            let now = crate::shared::utils::now();
            (now - last_heartbeat).num_minutes() < 5 // 5 minutes tolerance
        } else {
            false
        }
    }

    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Some(crate::shared::utils::now());
        self.updated_at = crate::shared::utils::now();
    }

    pub fn set_online(&mut self, fcm_token: Option<String>) {
        self.status = ProviderStatus::Online;
        self.fcm_token = fcm_token;
        self.update_heartbeat();
    }

    pub fn set_offline(&mut self) {
        self.status = ProviderStatus::Offline;
        self.current_load = 0;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn increment_load(&mut self) {
        self.current_load += 1;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn decrement_load(&mut self) {
        if self.current_load > 0 {
            self.current_load -= 1;
        }
        self.updated_at = crate::shared::utils::now();
    }

    pub fn record_message_sent(&mut self) {
        self.total_messages_sent += 1;
        self.messages_sent_today += 1;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn record_message_delivered(&mut self, reward_amount: f64) {
        self.total_messages_delivered += 1;
        self.earnings_total += reward_amount;
        self.update_success_rate();
        self.updated_at = crate::shared::utils::now();
    }

    pub fn record_message_failed(&mut self) {
        self.update_success_rate();
        self.updated_at = crate::shared::utils::now();
    }

    fn update_success_rate(&mut self) {
        if self.total_messages_sent > 0 {
            self.success_rate = (self.total_messages_delivered as f64 / self.total_messages_sent as f64) * 100.0;
        }
    }

    pub fn reset_daily_counters(&mut self) {
        self.messages_sent_today = 0;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn update_location(&mut self, location: Location) {
        self.location = Some(location);
        self.updated_at = crate::shared::utils::now();
    }
}
