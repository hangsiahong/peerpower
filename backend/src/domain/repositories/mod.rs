use async_trait::async_trait;
use crate::domain::entities::*;
use crate::shared::types::{Carrier, ProviderStatus, MessageStatus, PhoneNumber};
use crate::shared::Result;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>>;
    async fn find_by_phone(&self, phone: &PhoneNumber) -> Result<Option<User>>;
    async fn find_by_did(&self, did: &str) -> Result<Option<User>>;
    async fn update(&self, user: &User) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait ProviderRepository: Send + Sync {
    async fn create(&self, provider: &Provider) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Provider>>;
    async fn find_by_user_id(&self, user_id: &str) -> Result<Option<Provider>>;
    async fn find_available_by_carrier(&self, carrier: &Carrier) -> Result<Vec<Provider>>;
    async fn find_by_status(&self, status: &ProviderStatus) -> Result<Vec<Provider>>;
    async fn update(&self, provider: &Provider) -> Result<()>;
    async fn update_status(&self, id: &str, status: ProviderStatus) -> Result<()>;
    async fn update_heartbeat(&self, id: &str) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn find_stale_providers(&self, minutes: i64) -> Result<Vec<Provider>>;
}

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, message: &Message) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Message>>;
    async fn find_by_client_id(&self, client_id: &str, limit: Option<i64>) -> Result<Vec<Message>>;
    async fn find_pending_messages(&self, limit: Option<i64>) -> Result<Vec<Message>>;
    async fn find_by_status(&self, status: &MessageStatus) -> Result<Vec<Message>>;
    async fn update(&self, message: &Message) -> Result<()>;
    async fn update_status(&self, id: &str, status: MessageStatus) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn find_expired_messages(&self) -> Result<Vec<Message>>;
    async fn count_by_client_today(&self, client_id: &str) -> Result<i64>;
}

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create(&self, job: &Job) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Job>>;
    async fn find_by_message_id(&self, message_id: &str) -> Result<Option<Job>>;
    async fn find_by_provider_id(&self, provider_id: &str) -> Result<Vec<Job>>;
    async fn find_active_jobs(&self) -> Result<Vec<Job>>;
    async fn find_expired_jobs(&self) -> Result<Vec<Job>>;
    async fn update(&self, job: &Job) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
