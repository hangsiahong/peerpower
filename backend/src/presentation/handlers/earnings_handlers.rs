use axum::{
    extract::{Query, State},
    response::Json,
};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::domain::entities::Provider;
use crate::presentation::handlers::message_handlers::AuthenticatedUser;
use crate::shared::{AppState, PeerPowerError, Result};

#[derive(Debug, Deserialize)]
pub struct EarningsQuery {
    pub period: Option<String>, // "today", "week", "month", "all"
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct EarningsResponse {
    pub provider_id: String,
    pub total_earnings: f64,
    pub messages_delivered: u64,
    pub success_rate: f64,
    pub period: String,
    pub earnings_breakdown: EarningsBreakdown,
}

#[derive(Debug, Serialize)]
pub struct EarningsBreakdown {
    pub base_earnings: f64,
    pub priority_bonus: f64,
    pub volume_bonus: f64,
    pub quality_bonus: f64,
}

#[derive(Debug, Serialize)]
pub struct EarningsHistoryEntry {
    pub date: String,
    pub messages_count: u32,
    pub earnings: f64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct EarningsHistoryResponse {
    pub provider_id: String,
    pub period: String,
    pub total_earnings: f64,
    pub history: Vec<EarningsHistoryEntry>,
}

/// Get provider earnings summary
pub async fn get_provider_earnings(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EarningsQuery>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<EarningsResponse>> {
    info!("Getting earnings for user: {}", user_id);

    // Find the provider
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let provider = providers_collection
        .find_one(mongodb::bson::doc! {"user_id": &user_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch provider: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Provider for user: {}", user_id),
        })?;

    let period = params.period.unwrap_or_else(|| "all".to_string());

    // Calculate date range based on period
    let (start_date, end_date) = match period.as_str() {
        "today" => {
            let today = chrono::Utc::now().date_naive();
            let start = today.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = today.and_hms_opt(23, 59, 59).unwrap().and_utc();
            (Some(start), Some(end))
        }
        "week" => {
            let now = chrono::Utc::now();
            let week_ago = now - chrono::Duration::days(7);
            (Some(week_ago), Some(now))
        }
        "month" => {
            let now = chrono::Utc::now();
            let month_ago = now - chrono::Duration::days(30);
            (Some(month_ago), Some(now))
        }
        _ => (None, None), // "all"
    };

    // Build query for messages delivered by this provider
    let mut message_filter = mongodb::bson::doc! {
        "provider_id": &provider.id,
        "status": "Delivered"
    };

    if let (Some(start), Some(end)) = (start_date, end_date) {
        message_filter.insert(
            "updated_at",
            mongodb::bson::doc! {
                "$gte": start,
                "$lte": end
            },
        );
    }

    // Get delivered messages count and calculate earnings
    let messages_collection = app_state
        .database
        .collection::<crate::domain::entities::Message>("messages");
    let delivered_count = messages_collection
        .count_documents(message_filter.clone(), None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count delivered messages: {}", e),
        })? as u64;

    // Calculate total earnings (simplified - in real system this would be more complex)
    let total_earnings = if period == "all" {
        provider.earnings_total
    } else {
        // For time-based periods, we'd need to aggregate from message history
        // For now, use a simplified calculation
        delivered_count as f64 * 0.008 // Average earnings per message
    };

    // Calculate success rate
    let total_messages_filter = mongodb::bson::doc! {
        "provider_id": &provider.id
    };
    let total_messages = messages_collection
        .count_documents(total_messages_filter, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to count total messages: {}", e),
        })? as u64;

    let success_rate = if total_messages > 0 {
        (delivered_count as f64 / total_messages as f64) * 100.0
    } else {
        0.0
    };

    // Create earnings breakdown (simplified)
    let earnings_breakdown = EarningsBreakdown {
        base_earnings: total_earnings * 0.8,
        priority_bonus: total_earnings * 0.1,
        volume_bonus: total_earnings * 0.05,
        quality_bonus: total_earnings * 0.05,
    };

    Ok(Json(EarningsResponse {
        provider_id: provider.id,
        total_earnings,
        messages_delivered: delivered_count,
        success_rate,
        period,
        earnings_breakdown,
    }))
}

/// Get provider earnings history
pub async fn get_earnings_history(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<EarningsQuery>,
    AuthenticatedUser(user_id): AuthenticatedUser,
) -> Result<Json<EarningsHistoryResponse>> {
    info!("Getting earnings history for user: {}", user_id);

    // Find the provider
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let provider = providers_collection
        .find_one(mongodb::bson::doc! {"user_id": &user_id}, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to fetch provider: {}", e),
        })?
        .ok_or_else(|| PeerPowerError::NotFound {
            resource: format!("Provider for user: {}", user_id),
        })?;

    let period = params.period.unwrap_or_else(|| "month".to_string());

    // For now, create sample earnings history
    // In a real system, this would aggregate from a detailed earnings log table
    let mut history = Vec::new();
    let days_back = match period.as_str() {
        "week" => 7,
        "month" => 30,
        _ => 30,
    };

    for i in 0..days_back {
        let date = chrono::Utc::now() - chrono::Duration::days(i);
        let daily_messages = (provider.total_messages_delivered / days_back as u64).max(1);
        let daily_earnings = daily_messages as f64 * 0.008;
        let daily_success_rate = 95.0 + (i as f64 * 0.1); // Simulate variation

        history.push(EarningsHistoryEntry {
            date: date.format("%Y-%m-%d").to_string(),
            messages_count: daily_messages as u32,
            earnings: daily_earnings,
            success_rate: daily_success_rate.min(100.0),
        });
    }

    // Reverse to show oldest first
    history.reverse();

    let total_earnings: f64 = history.iter().map(|h| h.earnings).sum();

    Ok(Json(EarningsHistoryResponse {
        provider_id: provider.id,
        period,
        total_earnings,
        history,
    }))
}

/// Get system-wide earnings statistics (admin endpoint)
pub async fn get_system_earnings_stats(
    State(app_state): State<Arc<AppState>>,
    AuthenticatedUser(_user_id): AuthenticatedUser, // TODO: Add admin role check
) -> Result<Json<serde_json::Value>> {
    info!("Getting system earnings statistics");

    // Get total earnings across all providers
    let providers_collection = app_state.database.collection::<Provider>("providers");
    let pipeline = vec![mongodb::bson::doc! {
        "$group": {
            "_id": null,
            "total_earnings": { "$sum": "$earnings_total" },
            "total_messages": { "$sum": "$total_messages_delivered" },
            "total_providers": { "$sum": 1 },
            "avg_success_rate": { "$avg": "$success_rate" }
        }
    }];

    let mut cursor = providers_collection
        .aggregate(pipeline, None)
        .await
        .map_err(|e| PeerPowerError::Database {
            message: format!("Failed to aggregate earnings: {}", e),
        })?;

    let stats = if let Ok(Some(doc)) = cursor.try_next().await {
        serde_json::json!({
            "total_earnings": doc.get_f64("total_earnings").unwrap_or(0.0),
            "total_messages_delivered": doc.get_i64("total_messages").unwrap_or(0),
            "total_active_providers": doc.get_i32("total_providers").unwrap_or(0),
            "average_success_rate": doc.get_f64("avg_success_rate").unwrap_or(0.0),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    } else {
        serde_json::json!({
            "total_earnings": 0.0,
            "total_messages_delivered": 0,
            "total_active_providers": 0,
            "average_success_rate": 0.0,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    };

    Ok(Json(stats))
}
