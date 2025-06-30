use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::{ClientOptions, ResolverConfig},
    Client, Collection, Database,
};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{info, warn};

use crate::config::DatabaseConfig;
use crate::shared::{PeerPowerError, Result};

#[derive(Clone)]
pub struct MongoDatabase {
    client: Arc<Client>,
    database: Arc<Database>,
}

impl MongoDatabase {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to MongoDB at {}", config.url);

        // Parse MongoDB connection string
        let mut client_options = ClientOptions::parse_with_resolver_config(
            &config.url,
            ResolverConfig::cloudflare(),
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Invalid MongoDB URL: {}", e),
        })?;

        // Configure connection pool
        client_options.max_pool_size = Some(config.max_connections);
        client_options.min_pool_size = Some(config.min_connections);
        client_options.connect_timeout = Some(Duration::from_secs(config.connection_timeout_seconds));
        client_options.server_selection_timeout = Some(Duration::from_secs(10));

        // Set application name for monitoring
        client_options.app_name = Some("peerpower-backend".to_string());

        // Create client
        let client = Client::with_options(client_options)
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create MongoDB client: {}", e),
            })?;

        // Test connection - use the specified database, not admin
        let database = client.database(&config.name);
        database
            .run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to connect to MongoDB: {}", e),
            })?;

        info!("Successfully connected to MongoDB database: {}", config.name);

        Ok(Self {
            client: Arc::new(client),
            database: Arc::new(database),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn collection<T>(&self, collection_name: &str) -> Collection<T> {
        self.database.collection(collection_name)
    }

    pub async fn health_check(&self) -> Result<()> {
        self.database
            .run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Database health check failed: {}", e),
            })?;
        Ok(())
    }

    pub async fn create_indexes(&self) -> Result<()> {
        info!("Creating database indexes...");

        // Users collection indexes
        let users_collection: Collection<Document> = self.collection("users");
        
        // Unique index on phone number
        users_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"phone": 1})
                    .options(
                        mongodb::options::IndexOptions::builder()
                            .unique(true)
                            .build(),
                    )
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create users phone index: {}", e),
            })?;

        // Index on DID
        users_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"did": 1})
                    .options(
                        mongodb::options::IndexOptions::builder()
                            .sparse(true)
                            .build(),
                    )
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create users DID index: {}", e),
            })?;

        // Providers collection indexes
        let providers_collection: Collection<Document> = self.collection("providers");
        
        // Index on user_id
        providers_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"user_id": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create providers user_id index: {}", e),
            })?;

        // Compound index on carrier and status for provider matching
        providers_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"carrier": 1, "status": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create providers carrier-status index: {}", e),
            })?;

        // Index on last_heartbeat for cleanup
        providers_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"last_heartbeat": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create providers heartbeat index: {}", e),
            })?;

        // Messages collection indexes
        let messages_collection: Collection<Document> = self.collection("messages");
        
        // Index on client_id for client queries
        messages_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"client_id": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create messages client_id index: {}", e),
            })?;

        // Compound index on status and priority for message dispatch
        messages_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"status": 1, "priority": -1, "created_at": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create messages dispatch index: {}", e),
            })?;

        // Index on expires_at for cleanup
        messages_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"expires_at": 1})
                    .options(
                        mongodb::options::IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .build(),
                    )
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create messages expiry index: {}", e),
            })?;

        // Jobs collection indexes
        let jobs_collection: Collection<Document> = self.collection("jobs");
        
        // Index on message_id
        jobs_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"message_id": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create jobs message_id index: {}", e),
            })?;

        // Index on provider_id
        jobs_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"provider_id": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create jobs provider_id index: {}", e),
            })?;

        // Index on timeout_at for cleanup
        jobs_collection
            .create_index(
                mongodb::IndexModel::builder()
                    .keys(doc! {"timeout_at": 1})
                    .build(),
                None,
            )
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to create jobs timeout index: {}", e),
            })?;

        info!("Database indexes created successfully");
        Ok(())
    }
}
