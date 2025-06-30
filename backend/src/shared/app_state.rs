use std::sync::Arc;

use crate::config::AppConfig;
use crate::domain::repositories::UserRepository;
use crate::domain::services::AuthService;
use crate::infrastructure::auth_service_impl::AuthServiceImpl;
use crate::infrastructure::database::user_repository::MongoUserRepository;
use crate::infrastructure::messaging::fcm_service::{FcmService, FcmServiceImpl};
use crate::shared::Result;

// Application state for dependency injection
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub database: crate::infrastructure::database::MongoDatabase,
    pub redis: crate::infrastructure::database::RedisConnection,
    pub auth_service: Arc<dyn AuthService>,
    pub user_repository: Arc<dyn UserRepository>,
    pub fcm_service: Arc<dyn FcmService>,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self> {
        // Initialize database connections
        let database =
            crate::infrastructure::database::MongoDatabase::new(&config.database).await?;
        let redis = crate::infrastructure::database::RedisConnection::new(&config.redis).await?;

        // Create database indexes
        database.create_indexes().await?;

        // Create repositories
        let user_repo = Arc::new(MongoUserRepository::new(Arc::new(
            database.database().clone(),
        )));

        // Create auth service
        let auth_service: Arc<dyn AuthService> = Arc::new(AuthServiceImpl::new(
            config.auth.clone(),
            Arc::new(redis.clone()),
            user_repo.clone(),
        ));

        // Create FCM service
        let fcm_service: Arc<dyn FcmService> =
            Arc::new(FcmServiceImpl::new(config.external.fcm.clone()));

        Ok(Self {
            config,
            database,
            redis,
            auth_service,
            user_repository: user_repo,
            fcm_service,
        })
    }
}
