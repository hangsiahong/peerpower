use axum::{
    extract::{Query, State},
    response::Json,
};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::domain::entities::{Message, Provider};
use crate::presentation::handlers::message_handlers::AuthenticatedUser;
use crate::shared::{AppState, PeerPowerError, Result};

#[derive(Debug, Deserialize)]
pub struct AdminStatsQuery {
    pub period: Option<String>, // "today", "week", "month", "all"
}

#[derive(Debug, Serialize)]
pub struct SystemStatsResponse {
    pub total_users: u64,
    pub total_providers: u64,
    pub active_providers: u64,
    pub total_messages: u64,
    pub messages_delivered: u64,
    pub messages_failed: u64,
    pub system_success_rate: f64,
    pub total_earnings_distributed: f64,
    pub average_message_cost: f64,
    pub period: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatsEntry {
    pub provider_id: String,
    pub user_id: String,
    pub phone: String,
    pub carrier: String,
    pub status: String,
    pub messages_delivered: u64,
    pub success_rate: f64,
    pub total_earnings: f64,
    pub last_active: String,
}

#[derive(Debug, Serialize)]
pub struct MessageStatsEntry {
    pub date: String,
    pub total_messages: u32,
    pub delivered_messages: u32,
    pub failed_messages: u32,
    pub success_rate: f64,
    pub total_cost: f64,
}

/// Get system statistics (admin only)
pub async fn get_system_stats(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<AdminStatsQuery>,
    AuthenticatedUser(_user_id): AuthenticatedUser, // TODO: Add admin role validation
) -> Result<Json<SystemStatsResponse>> {
    info!("Getting system statistics");

    let period = params.period.unwrap_or_else(|| "all".to_string());

    // Calculate date range based on period
    let date_filter = match period.as_str() {
        "today" => {
            let today = chrono::Utc::now().date_naive();
            let start = today.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = today.and_hms_opt(23, 59, 59).unwrap().and_utc();
            Some(mongodb::bson::doc! {
                "created_at": {
                    "$gte": start,
                    "$lte": end
                }
            })
        }
        "week" => {
            let now = chrono::Utc::now();
            let week_ago = now - chrono::Duration::days(7);
            Some(mongodb::bson::doc! {
                "created_at": {
                    "$gte": week_ago,
                    "$lte": now
                }
            })
        }
        "month" => {
            let now = chrono::Utc::now();
            let month_ago = now - chrono::Duration::days(30);
            Some(mongodb::bson::doc! {
                "created_at": {
                    "$gte": month_ago,
                    "$lte": now
                }
            })
        }
        _ => None, // "all"
    };

    // Get user count
    let users_collection = app_state
        .database
        .collection::<mongodb::bson::Document>("users");
    let total_users = users_collection
        .count_documents(mongodb::bson::doc! {}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count users: {}", e),
        })?;

    // Get provider stats
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let total_providers = providers_collection
        .count_documents(mongodb::bson::doc! {}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count providers: {}", e),
        })?;

    let active_providers = providers_collection
        .count_documents(
            mongodb::bson::doc! {
                "status": { "$in": ["Online", "Busy"] }
            },
            None,
        )
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count active providers: {}", e),
        })?;

    // Get message stats
    let messages_collection = app_state.database.collection::<Message>("messages");
    let message_filter = date_filter.unwrap_or_else(|| mongodb::bson::doc! {});

    let total_messages = messages_collection
        .count_documents(message_filter.clone(), None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count messages: {}", e),
        })?;

    let mut delivered_filter = message_filter.clone();
    delivered_filter.insert("status", "Delivered");
    let messages_delivered = messages_collection
        .count_documents(delivered_filter, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count delivered messages: {}", e),
        })?;

    let mut failed_filter = message_filter.clone();
    failed_filter.insert("status", "Failed");
    let messages_failed = messages_collection
        .count_documents(failed_filter, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count failed messages: {}", e),
        })?;

    // Calculate success rate
    let system_success_rate = if total_messages > 0 {
        (messages_delivered as f64 / total_messages as f64) * 100.0
    } else {
        0.0
    };

    // Get total earnings (aggregate from providers)
    let earnings_pipeline = vec![mongodb::bson::doc! {
        "$group": {
            "_id": null,
            "total_earnings": { "$sum": "$earnings_total" }
        }
    }];

    let mut earnings_cursor = providers_collection
        .aggregate(earnings_pipeline, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to aggregate earnings: {}", e),
        })?;

    let total_earnings_distributed = if let Ok(Some(doc)) = earnings_cursor.try_next().await {
        doc.get_f64("total_earnings").unwrap_or(0.0)
    } else {
        0.0
    };

    // Calculate average message cost
    let average_message_cost = if total_messages > 0 {
        (total_earnings_distributed * 1.25) / total_messages as f64 // Add 25% for platform fee
    } else {
        0.0
    };

    Ok(Json(SystemStatsResponse {
        total_users,
        total_providers,
        active_providers,
        total_messages,
        messages_delivered,
        messages_failed,
        system_success_rate,
        total_earnings_distributed,
        average_message_cost,
        period,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

/// Get provider performance stats (admin only)
pub async fn get_provider_performance(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<AdminStatsQuery>,
    AuthenticatedUser(_user_id): AuthenticatedUser, // TODO: Add admin role validation
) -> Result<Json<Vec<ProviderStatsEntry>>> {
    info!("Getting provider performance stats");

    let providers_collection = app_state.database.collection::<Provider>("providers");
    let mut cursor = providers_collection
        .find(mongodb::bson::doc! {}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch providers: {}", e),
        })?;

    let mut provider_stats = Vec::new();
    while let Ok(Some(provider)) = cursor.try_next().await {
        provider_stats.push(ProviderStatsEntry {
            provider_id: provider.id.clone(),
            user_id: provider.user_id.clone(),
            phone: provider.phone.as_str().to_string(),
            carrier: format!("{:?}", provider.carrier),
            status: format!("{:?}", provider.status),
            messages_delivered: provider.total_messages_delivered,
            success_rate: provider.success_rate,
            total_earnings: provider.earnings_total,
            last_active: provider
                .last_heartbeat
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "Never".to_string()),
        });
    }

    // Sort by total earnings (highest first)
    provider_stats.sort_by(|a, b| b.total_earnings.partial_cmp(&a.total_earnings).unwrap());

    Ok(Json(provider_stats))
}

/// Get message analytics (admin only)
pub async fn get_message_analytics(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<AdminStatsQuery>,
    AuthenticatedUser(_user_id): AuthenticatedUser, // TODO: Add admin role validation
) -> Result<Json<Vec<MessageStatsEntry>>> {
    info!("Getting message analytics");

    let period = params.period.unwrap_or_else(|| "month".to_string());
    let days_back = match period.as_str() {
        "week" => 7,
        "month" => 30,
        _ => 30,
    };

    let mut analytics = Vec::new();
    let messages_collection = app_state.database.collection::<Message>("messages");

    for i in 0..days_back {
        let date = chrono::Utc::now() - chrono::Duration::days(i);
        let start_of_day = date.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_of_day = date.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();

        let day_filter = mongodb::bson::doc! {
            "created_at": {
                "$gte": start_of_day,
                "$lte": end_of_day
            }
        };

        let total_messages = messages_collection
            .count_documents(day_filter.clone(), None)
            .await
            .unwrap_or(0) as u32;

        let mut delivered_filter = day_filter.clone();
        delivered_filter.insert("status", "Delivered");
        let delivered_messages = messages_collection
            .count_documents(delivered_filter, None)
            .await
            .unwrap_or(0) as u32;

        let mut failed_filter = day_filter.clone();
        failed_filter.insert("status", "Failed");
        let failed_messages = messages_collection
            .count_documents(failed_filter, None)
            .await
            .unwrap_or(0) as u32;

        let success_rate = if total_messages > 0 {
            (delivered_messages as f64 / total_messages as f64) * 100.0
        } else {
            0.0
        };

        let total_cost = total_messages as f64 * 0.01; // Simplified cost calculation

        analytics.push(MessageStatsEntry {
            date: date.format("%Y-%m-%d").to_string(),
            total_messages,
            delivered_messages,
            failed_messages,
            success_rate,
            total_cost,
        });
    }

    // Reverse to show oldest first
    analytics.reverse();

    Ok(Json(analytics))
}
