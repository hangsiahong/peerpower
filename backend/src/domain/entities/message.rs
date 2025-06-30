use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::shared::types::{PhoneNumber, Carrier, MessageStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub client_id: String,
    pub content: String,
    pub recipient: PhoneNumber,
    pub recipient_carrier: Carrier,
    pub provider_id: Option<String>,
    pub status: MessageStatus,
    pub priority: MessagePriority,
    pub metadata: MessageMetadata,
    pub delivery_report: Option<DeliveryReport>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub tracking_id: String,
    pub client_reference: Option<String>,
    pub webhook_url: Option<String>,
    pub max_retries: u32,
    pub retry_count: u32,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryReport {
    pub delivered_at: DateTime<Utc>,
    pub provider_confirmation: bool,
    pub delivery_status: String,
    pub error_message: Option<String>,
    pub network_info: Option<NetworkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub carrier: Carrier,
    pub signal_strength: Option<i32>,
    pub network_type: Option<String>,
}

impl Message {
    pub fn new(
        client_id: String,
        content: String,
        recipient: PhoneNumber,
        priority: MessagePriority,
        client_reference: Option<String>,
        webhook_url: Option<String>,
    ) -> Self {
        let now = crate::shared::utils::now();
        let recipient_carrier = Carrier::from_phone_number(&recipient);
        
        Self {
            id: crate::shared::utils::generate_id(),
            client_id,
            content,
            recipient,
            recipient_carrier,
            provider_id: None,
            status: MessageStatus::Pending,
            priority,
            metadata: MessageMetadata {
                tracking_id: crate::shared::utils::generate_id(),
                client_reference,
                webhook_url,
                max_retries: 3,
                retry_count: 0,
                region: None,
            },
            delivery_report: None,
            created_at: now,
            updated_at: now,
            scheduled_at: None,
            expires_at: Some(now + chrono::Duration::hours(24)), // 24 hour expiration
        }
    }

    pub fn assign_to_provider(&mut self, provider_id: String) {
        self.provider_id = Some(provider_id);
        self.status = MessageStatus::Assigned;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn mark_sent(&mut self) {
        self.status = MessageStatus::Sent;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn mark_delivered(&mut self, delivery_report: DeliveryReport) {
        self.status = MessageStatus::Delivered;
        self.delivery_report = Some(delivery_report);
        self.updated_at = crate::shared::utils::now();
    }

    pub fn mark_failed(&mut self, error_message: String) {
        self.status = MessageStatus::Failed;
        self.delivery_report = Some(DeliveryReport {
            delivered_at: crate::shared::utils::now(),
            provider_confirmation: false,
            delivery_status: "failed".to_string(),
            error_message: Some(error_message),
            network_info: None,
        });
        self.updated_at = crate::shared::utils::now();
    }

    pub fn increment_retry(&mut self) {
        self.metadata.retry_count += 1;
        self.status = MessageStatus::Pending;
        self.updated_at = crate::shared::utils::now();
    }

    pub fn can_retry(&self) -> bool {
        self.metadata.retry_count < self.metadata.max_retries 
            && !self.is_expired()
            && matches!(self.status, MessageStatus::Failed)
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            crate::shared::utils::now() > expires_at
        } else {
            false
        }
    }

    pub fn get_priority_score(&self) -> u32 {
        match self.priority {
            MessagePriority::Urgent => 100,
            MessagePriority::High => 75,
            MessagePriority::Normal => 50,
            MessagePriority::Low => 25,
        }
    }

    pub fn validate_content(&self) -> Result<(), String> {
        if self.content.is_empty() {
            return Err("Message content cannot be empty".to_string());
        }
        
        if self.content.len() > 160 {
            return Err("Message content exceeds 160 characters".to_string());
        }
        
        // Check for potential spam patterns
        if self.content.chars().filter(|c| c.is_ascii_digit()).count() == 6 
            && self.content.chars().all(|c| c.is_ascii_digit() || c.is_whitespace()) {
            return Err("OTP-like messages are not supported".to_string());
        }
        
        Ok(())
    }

    pub fn is_deliverable(&self) -> bool {
        matches!(self.status, MessageStatus::Pending | MessageStatus::Assigned)
            && !self.is_expired()
            && self.validate_content().is_ok()
    }
}
