use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::warn;

use crate::domain::services::{AuthService, TokenClaims};
use crate::shared::{PeerPowerError, AppState};

/// JWT authentication middleware
pub async fn auth_middleware<B>(
    State(app_state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract token from Authorization header
    let token = extract_token(&request)?;
    
    // Validate token
    let claims = app_state.auth_service
        .validate_token(&token)
        .await
        .map_err(|e| {
            warn!("Token validation failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    // Add claims to request extensions for handlers to use
    request.extensions_mut().insert(claims);
    
    Ok(next.run(request).await)
}

/// Extract Bearer token from Authorization header
fn extract_token(request: &Request) -> Result<String, StatusCode> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let token = auth_header.trim_start_matches("Bearer ").to_string();
    
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(token)
}

/// Extension trait to get token claims from request
pub trait RequestExt {
    fn token_claims(&self) -> Option<&TokenClaims>;
    fn user_id(&self) -> Option<&str>;
    fn is_provider(&self) -> bool;
}

impl RequestExt for Request {
    fn token_claims(&self) -> Option<&TokenClaims> {
        self.extensions().get::<TokenClaims>()
    }
    
    fn user_id(&self) -> Option<&str> {
        self.token_claims().map(|claims| claims.sub.as_str())
    }
    
    fn is_provider(&self) -> bool {
        self.token_claims()
            .map(|claims| claims.is_provider)
            .unwrap_or(false)
    }
}
