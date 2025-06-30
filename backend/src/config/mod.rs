use crate::shared::PeerPowerError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub external: ExternalServicesConfig,
    pub instance: InstanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub environment: Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub name: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub otp_expiration_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServicesConfig {
    pub fcm: FcmConfig,
    pub baray: BarayConfig,
    pub selendra: SelendraConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FcmConfig {
    pub server_key: String,
    pub sender_id: String,
    pub fcm_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarayConfig {
    pub api_key: String,
    pub webhook_secret: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelendraConfig {
    pub rpc_url: String,
    pub private_key: String,
    pub token_contract_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: String,
    pub region: String,
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl AppConfig {
    /// Load configuration from environment variables and config files
    pub fn from_env() -> Result<Self, PeerPowerError> {
        // Load from environment first
        dotenvy::dotenv().ok(); // Don't fail if .env doesn't exist

        let config = AppConfig {
            server: ServerConfig {
                host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: std::env::var("PORT")
                    .unwrap_or_else(|_| "8080".to_string())
                    .parse()
                    .map_err(|_| PeerPowerError::Configuration {
                        message: "Invalid PORT value".to_string(),
                    })?,
                environment: match std::env::var("ENVIRONMENT")
                    .unwrap_or_else(|_| "development".to_string())
                    .to_lowercase()
                    .as_str()
                {
                    "production" => Environment::Production,
                    "staging" => Environment::Staging,
                    _ => Environment::Development,
                },
            },
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL").map_err(|_| PeerPowerError::Configuration {
                    message: "DATABASE_URL is required".to_string(),
                })?,
                name: std::env::var("DATABASE_NAME").unwrap_or_else(|_| "peerpower".to_string()),
                max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
                min_connections: std::env::var("DATABASE_MIN_CONNECTIONS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .unwrap_or(5),
                connection_timeout_seconds: std::env::var("DATABASE_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            },
            redis: RedisConfig {
                url: std::env::var("REDIS_URL").map_err(|_| PeerPowerError::Configuration {
                    message: "REDIS_URL is required".to_string(),
                })?,
                max_connections: std::env::var("REDIS_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "50".to_string())
                    .parse()
                    .unwrap_or(50),
                connection_timeout_seconds: std::env::var("REDIS_TIMEOUT")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            },
            auth: AuthConfig {
                jwt_secret: std::env::var("JWT_SECRET").map_err(|_| {
                    PeerPowerError::Configuration {
                        message: "JWT_SECRET is required".to_string(),
                    }
                })?,
                jwt_expiration_hours: std::env::var("JWT_EXPIRATION_HOURS")
                    .unwrap_or_else(|_| "24".to_string())
                    .parse()
                    .unwrap_or(24),
                otp_expiration_minutes: std::env::var("OTP_EXPIRATION_MINUTES")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .unwrap_or(5),
            },
            external: ExternalServicesConfig {
                fcm: FcmConfig {
                    server_key: std::env::var("FCM_SERVER_KEY").unwrap_or_default(),
                    sender_id: std::env::var("FCM_SENDER_ID").unwrap_or_default(),
                    fcm_url: std::env::var("FCM_URL")
                        .unwrap_or_else(|_| "https://fcm.googleapis.com/fcm/send".to_string()),
                },
                baray: BarayConfig {
                    api_key: std::env::var("BARAY_API_KEY").unwrap_or_default(),
                    webhook_secret: std::env::var("BARAY_WEBHOOK_SECRET").unwrap_or_default(),
                    base_url: std::env::var("BARAY_BASE_URL")
                        .unwrap_or_else(|_| "https://api.baray.io".to_string()),
                },
                selendra: SelendraConfig {
                    rpc_url: std::env::var("SELENDRA_RPC_URL")
                        .unwrap_or_else(|_| "https://rpc.selendra.org".to_string()),
                    private_key: std::env::var("SELENDRA_PRIVATE_KEY").unwrap_or_default(),
                    token_contract_address: std::env::var("PPT_CONTRACT_ADDRESS")
                        .unwrap_or_default(),
                },
            },
            instance: InstanceConfig {
                id: std::env::var("INSTANCE_ID")
                    .unwrap_or_else(|_| crate::shared::utils::generate_id()),
                region: std::env::var("REGION").unwrap_or_else(|_| "unknown".to_string()),
                zone: std::env::var("ZONE").ok(),
            },
        };

        Ok(config)
    }

    /// Check if we're in development mode
    pub fn is_development(&self) -> bool {
        matches!(self.server.environment, Environment::Development)
    }

    /// Check if we're in production mode
    pub fn is_production(&self) -> bool {
        matches!(self.server.environment, Environment::Production)
    }

    /// Get server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
