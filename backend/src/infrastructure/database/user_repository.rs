use async_trait::async_trait;
use bson::{doc, oid::ObjectId, Document};
use mongodb::{Collection, Database};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::entities::User;
use crate::domain::repositories::UserRepository;
use crate::shared::types::PhoneNumber;
use crate::shared::{PeerPowerError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: String,
    pub phone: String,
    pub did: Option<String>,
    pub evm_address: Option<String>,
    pub reputation_score: f64,
    pub is_provider: bool,
    pub is_verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<&User> for UserDocument {
    fn from(user: &User) -> Self {
        Self {
            id: None, // Let MongoDB generate the ObjectId
            user_id: user.id.clone(),
            phone: user.phone.as_str().to_string(),
            did: user.did.clone(),
            evm_address: user.evm_address.clone(),
            reputation_score: user.reputation_score,
            is_provider: user.is_provider,
            is_verified: user.is_verified,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl TryFrom<UserDocument> for User {
    type Error = PeerPowerError;

    fn try_from(doc: UserDocument) -> Result<Self> {
        let phone = PhoneNumber::new(doc.phone)?;

        Ok(User {
            id: doc.user_id,
            phone,
            did: doc.did,
            evm_address: doc.evm_address,
            reputation_score: doc.reputation_score,
            is_provider: doc.is_provider,
            is_verified: doc.is_verified,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
        })
    }
}

pub struct MongoUserRepository {
    collection: Collection<UserDocument>,
}

impl MongoUserRepository {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            collection: database.collection("users"),
        }
    }
}

#[async_trait]
impl UserRepository for MongoUserRepository {
    async fn create(&self, user: &User) -> Result<()> {
        let doc = UserDocument::from(user);

        tracing::info!(
            "Creating user in database with user_id: {} and phone: {}",
            doc.user_id,
            doc.phone
        );

        let result = self.collection.insert_one(doc, None).await.map_err(|e| {
            tracing::error!("Database insert error: {}", e);
            if e.to_string().contains("duplicate key") {
                PeerPowerError::ValidationError {
                    field: "phone".to_string(),
                    message: "Phone number already exists".to_string(),
                }
            } else {
                PeerPowerError::Database {
                    message: format!("Failed to create user: {}", e),
                }
            }
        })?;

        tracing::info!(
            "User created successfully with MongoDB _id: {:?}",
            result.inserted_id
        );
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>> {
        tracing::info!("Looking up user by ID: {}", id);
        let doc = self
            .collection
            .find_one(doc! {"user_id": id}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to find user by id: {}", e),
            })?;

        match doc {
            Some(user_doc) => {
                tracing::info!("Found user by ID: {}", id);
                Ok(Some(user_doc.try_into()?))
            }
            None => {
                tracing::info!("User not found by ID: {}", id);
                Ok(None)
            }
        }
    }

    async fn find_by_phone(&self, phone: &PhoneNumber) -> Result<Option<User>> {
        tracing::info!("Looking up user by phone: {}", phone.as_str());
        let doc = self
            .collection
            .find_one(doc! {"phone": phone.as_str()}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to find user by phone: {}", e),
            })?;

        match doc {
            Some(user_doc) => {
                tracing::info!(
                    "Found user by phone: {} with user_id: {}",
                    phone.as_str(),
                    user_doc.user_id
                );
                Ok(Some(user_doc.try_into()?))
            }
            None => {
                tracing::info!("User not found by phone: {}", phone.as_str());
                Ok(None)
            }
        }
    }

    async fn find_by_did(&self, did: &str) -> Result<Option<User>> {
        let doc = self
            .collection
            .find_one(doc! {"did": did}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to find user by DID: {}", e),
            })?;

        match doc {
            Some(user_doc) => Ok(Some(user_doc.try_into()?)),
            None => Ok(None),
        }
    }

    async fn update(&self, user: &User) -> Result<()> {
        let doc = UserDocument::from(user);

        let update_doc = doc! {
            "$set": {
                "did": &doc.did,
                "evm_address": &doc.evm_address,
                "reputation_score": doc.reputation_score,
                "is_provider": doc.is_provider,
                "is_verified": doc.is_verified,
                "updated_at": doc.updated_at
            }
        };

        let result = self
            .collection
            .update_one(doc! {"user_id": &user.id}, update_doc, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to update user: {}", e),
            })?;

        if result.matched_count == 0 {
            return Err(PeerPowerError::NotFound {
                resource: format!("User with id: {}", user.id),
            });
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let result = self
            .collection
            .delete_one(doc! {"user_id": id}, None)
            .await
            .map_err(|e| PeerPowerError::Database {
                message: format!("Failed to delete user: {}", e),
            })?;

        if result.deleted_count == 0 {
            return Err(PeerPowerError::NotFound {
                resource: format!("User with id: {}", id),
            });
        }

        Ok(())
    }
}
