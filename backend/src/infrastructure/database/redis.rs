use redis::{Client, Commands, Connection, RedisResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::RedisConfig;
use crate::shared::{PeerPowerError, Result};

#[derive(Clone)]
pub struct RedisConnection {
    client: Arc<Client>,
    connection: Arc<Mutex<Connection>>,
}

impl RedisConnection {
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        info!("Connecting to Redis at {}", config.url);

        let client =
            Client::open(config.url.as_str()).map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Failed to create Redis client: {}", e),
            })?;

        let connection = client
            .get_connection()
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Failed to connect to Redis: {}", e),
            })?;

        // Test connection
        let mut conn = connection;
        redis::cmd("PING").query::<String>(&mut conn).map_err(|e| {
            PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis ping failed: {}", e),
            }
        })?;

        info!("Successfully connected to Redis");

        Ok(Self {
            client: Arc::new(client),
            connection: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn health_check(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        redis::cmd("PING")
            .query::<String>(&mut *conn)
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis health check failed: {}", e),
            })?;
        Ok(())
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        expiration_seconds: Option<usize>,
    ) -> Result<()> {
        let mut conn = self.connection.lock().await;

        if let Some(exp) = expiration_seconds {
            redis::cmd("SETEX")
                .arg(key)
                .arg(exp as u64)
                .arg(value)
                .query::<()>(&mut *conn)
                .map_err(|e| PeerPowerError::ExternalService {
                    service: "Redis".to_string(),
                    message: format!("Redis SETEX failed: {}", e),
                })?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query::<()>(&mut *conn)
                .map_err(|e| PeerPowerError::ExternalService {
                    service: "Redis".to_string(),
                    message: format!("Redis SET failed: {}", e),
                })?;
        }

        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.connection.lock().await;

        let result: Option<String> = redis::cmd("GET").arg(key).query(&mut *conn).map_err(|e| {
            PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis GET failed: {}", e),
            }
        })?;

        Ok(result)
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.lock().await;

        let result: i32 = redis::cmd("DEL").arg(key).query(&mut *conn).map_err(|e| {
            PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis DEL failed: {}", e),
            }
        })?;

        Ok(result > 0)
    }

    pub async fn increment(&self, key: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;

        let result: i64 = redis::cmd("INCR").arg(key).query(&mut *conn).map_err(|e| {
            PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis INCR failed: {}", e),
            }
        })?;

        Ok(result)
    }

    pub async fn acquire_lock(&self, key: &str, ttl_seconds: usize) -> Result<bool> {
        let mut conn = self.connection.lock().await;

        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("locked")
            .arg("NX")
            .arg("EX")
            .arg(ttl_seconds as u64)
            .query(&mut *conn)
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis lock acquisition failed: {}", e),
            })?;

        Ok(result.is_some())
    }

    pub async fn release_lock(&self, key: &str) -> Result<()> {
        self.delete(key).await?;
        Ok(())
    }

    pub async fn lpush(&self, key: &str, value: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;

        let result: i64 = redis::cmd("LPUSH")
            .arg(key)
            .arg(value)
            .query(&mut *conn)
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis LPUSH failed: {}", e),
            })?;

        Ok(result)
    }

    pub async fn rpop(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.connection.lock().await;

        let result: Option<String> =
            redis::cmd("RPOP").arg(key).query(&mut *conn).map_err(|e| {
                PeerPowerError::ExternalService {
                    service: "Redis".to_string(),
                    message: format!("Redis RPOP failed: {}", e),
                }
            })?;

        Ok(result)
    }

    pub async fn brpop(&self, keys: &[&str], timeout: usize) -> Result<Option<(String, String)>> {
        let mut conn = self.connection.lock().await;

        let mut cmd = redis::cmd("BRPOP");
        for key in keys {
            cmd.arg(*key);
        }
        cmd.arg(timeout as u64);

        let result: Option<(String, String)> =
            cmd.query(&mut *conn)
                .map_err(|e| PeerPowerError::ExternalService {
                    service: "Redis".to_string(),
                    message: format!("Redis BRPOP failed: {}", e),
                })?;

        Ok(result)
    }

    pub async fn sadd(&self, key: &str, value: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;

        let result: i64 = redis::cmd("SADD")
            .arg(key)
            .arg(value)
            .query(&mut *conn)
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis SADD failed: {}", e),
            })?;

        Ok(result)
    }

    pub async fn srem(&self, key: &str, value: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;

        let result: i64 = redis::cmd("SREM")
            .arg(key)
            .arg(value)
            .query(&mut *conn)
            .map_err(|e| PeerPowerError::ExternalService {
                service: "Redis".to_string(),
                message: format!("Redis SREM failed: {}", e),
            })?;

        Ok(result)
    }

    pub async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        let mut conn = self.connection.lock().await;

        let result: Vec<String> =
            redis::cmd("SMEMBERS")
                .arg(key)
                .query(&mut *conn)
                .map_err(|e| PeerPowerError::ExternalService {
                    service: "Redis".to_string(),
                    message: format!("Redis SMEMBERS failed: {}", e),
                })?;

        Ok(result)
    }
}
