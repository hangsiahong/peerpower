use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::config::FcmConfig;
use crate::shared::{PeerPowerError, Result};

#[derive(Debug, Serialize)]
pub struct FcmMessage {
    pub to: String,
    pub data: HashMap<String, String>,
    pub notification: Option<FcmNotification>,
    pub priority: String,
    pub time_to_live: u32,
}

#[derive(Debug, Serialize)]
pub struct FcmNotification {
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
    pub sound: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FcmResponse {
    pub multicast_id: Option<u64>,
    pub success: u32,
    pub failure: u32,
    pub canonical_ids: u32,
    pub results: Option<Vec<FcmResult>>,
}

#[derive(Debug, Deserialize)]
pub struct FcmResult {
    pub message_id: Option<String>,
    pub registration_id: Option<String>,
    pub error: Option<String>,
}

#[async_trait]
pub trait FcmService: Send + Sync {
    async fn send_sms_dispatch_request(
        &self,
        fcm_token: &str,
        message_id: &str,
        recipient: &str,
        content: &str,
        priority: &str,
    ) -> Result<String>;

    async fn send_delivery_confirmation_request(
        &self,
        fcm_token: &str,
        message_id: &str,
        delivery_status: &str,
    ) -> Result<String>;

    async fn send_provider_status_update(&self, fcm_token: &str, status: &str) -> Result<String>;
}

pub struct FcmServiceImpl {
    config: FcmConfig,
    client: Client,
}

impl FcmServiceImpl {
    pub fn new(config: FcmConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    async fn send_fcm_message(&self, message: FcmMessage) -> Result<FcmResponse> {
        info!("Sending FCM message to: {}", message.to);

        let response = self
            .client
            .post(&self.config.fcm_url)
            .header("Authorization", format!("key={}", self.config.server_key))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
            .map_err(|e| PeerPowerError::ExternalService {
                service: "FCM".to_string(),
                message: format!("Failed to send FCM request: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("FCM request failed with status {}: {}", status, body);
            return Err(PeerPowerError::ExternalService {
                service: "FCM".to_string(),
                message: format!("FCM returned error {}: {}", status, body),
            });
        }

        let fcm_response: FcmResponse =
            response
                .json()
                .await
                .map_err(|e| PeerPowerError::ExternalService {
                    service: "FCM".to_string(),
                    message: format!("Failed to parse FCM response: {}", e),
                })?;

        // Check for errors in the response
        if fcm_response.failure > 0 {
            if let Some(results) = &fcm_response.results {
                for result in results {
                    if let Some(error) = &result.error {
                        warn!("FCM delivery error: {}", error);
                    }
                }
            }
        }

        info!(
            "FCM message sent successfully - Success: {}, Failure: {}",
            fcm_response.success, fcm_response.failure
        );

        Ok(fcm_response)
    }
}

#[async_trait]
impl FcmService for FcmServiceImpl {
    async fn send_sms_dispatch_request(
        &self,
        fcm_token: &str,
        message_id: &str,
        recipient: &str,
        content: &str,
        priority: &str,
    ) -> Result<String> {
        let mut data = HashMap::new();
        data.insert("type".to_string(), "sms_dispatch".to_string());
        data.insert("message_id".to_string(), message_id.to_string());
        data.insert("recipient".to_string(), recipient.to_string());
        data.insert("content".to_string(), content.to_string());
        data.insert("priority".to_string(), priority.to_string());

        let message = FcmMessage {
            to: fcm_token.to_string(),
            data,
            notification: Some(FcmNotification {
                title: "New SMS Request".to_string(),
                body: format!("Send SMS to {}", recipient),
                icon: Some("ic_sms".to_string()),
                sound: Some("default".to_string()),
            }),
            priority: if priority == "High" { "high" } else { "normal" }.to_string(),
            time_to_live: 300, // 5 minutes
        };

        let response = self.send_fcm_message(message).await?;

        if response.success > 0 {
            Ok("FCM dispatch request sent successfully".to_string())
        } else {
            Err(PeerPowerError::ExternalService {
                service: "FCM".to_string(),
                message: "Failed to deliver FCM message".to_string(),
            })
        }
    }

    async fn send_delivery_confirmation_request(
        &self,
        fcm_token: &str,
        message_id: &str,
        delivery_status: &str,
    ) -> Result<String> {
        let mut data = HashMap::new();
        data.insert("type".to_string(), "delivery_confirmation".to_string());
        data.insert("message_id".to_string(), message_id.to_string());
        data.insert("status".to_string(), delivery_status.to_string());

        let message = FcmMessage {
            to: fcm_token.to_string(),
            data,
            notification: None, // Silent message for delivery confirmations
            priority: "normal".to_string(),
            time_to_live: 60, // 1 minute
        };

        let response = self.send_fcm_message(message).await?;

        if response.success > 0 {
            Ok("Delivery confirmation request sent successfully".to_string())
        } else {
            Err(PeerPowerError::ExternalService {
                service: "FCM".to_string(),
                message: "Failed to deliver delivery confirmation".to_string(),
            })
        }
    }

    async fn send_provider_status_update(&self, fcm_token: &str, status: &str) -> Result<String> {
        let mut data = HashMap::new();
        data.insert("type".to_string(), "status_update".to_string());
        data.insert("status".to_string(), status.to_string());

        let message = FcmMessage {
            to: fcm_token.to_string(),
            data,
            notification: None, // Silent message for status updates
            priority: "normal".to_string(),
            time_to_live: 120, // 2 minutes
        };

        let response = self.send_fcm_message(message).await?;

        if response.success > 0 {
            Ok("Status update sent successfully".to_string())
        } else {
            Err(PeerPowerError::ExternalService {
                service: "FCM".to_string(),
                message: "Failed to deliver status update".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fcm_config_creation() {
        let config = FcmConfig {
            server_key: "test_server_key".to_string(),
            sender_id: "test_sender_id".to_string(),
            fcm_url: "https://fcm.googleapis.com/fcm/send".to_string(),
        };
        assert_eq!(config.server_key, "test_server_key");
        assert_eq!(config.fcm_url, "https://fcm.googleapis.com/fcm/send");
    }

    #[tokio::test]
    async fn test_fcm_message_serialization() {
        let mut data = HashMap::new();
        data.insert("type".to_string(), "test".to_string());

        let message = FcmMessage {
            to: "test_token".to_string(),
            data,
            notification: None,
            priority: "normal".to_string(),
            time_to_live: 60,
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("normal"));
    }
}
