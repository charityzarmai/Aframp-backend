use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::jwt::{
    blacklist_access_token, generate_access_token, generate_refresh_token, revoke_refresh_token,
    store_refresh_token, validate_token, JwtError, RefreshTokenRecord, Scope, TokenType,
};
use crate::cache::RedisCache;

#[derive(Clone)]
pub struct AuthHandlerState {
    pub jwt_secret: String,
    pub redis_cache: Option<RedisCache>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateTokenRequest {
    pub wallet_address: String,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeTokenRequest {
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

fn parse_scope(scope: Option<&str>) -> Scope {
    match scope.unwrap_or("user") {
        "admin" => Scope::Admin,
        _ => Scope::User,
    }
}

fn jwt_error_response(err: JwtError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match err {
        JwtError::MissingToken | JwtError::InvalidToken | JwtError::TokenExpired | JwtError::TokenRevoked => StatusCode::UNAUTHORIZED,
        JwtError::InsufficientPermissions { .. } => StatusCode::FORBIDDEN,
        JwtError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (
        status,
        Json(json!({
            "error": err.to_string(),
        })),
    )
}

pub async fn generate_token(
    State(state): State<Arc<AuthHandlerState>>,
    Json(req): Json<GenerateTokenRequest>,
) -> impl IntoResponse {
    let scope = parse_scope(req.scope.as_deref());
    let (access_token, access_claims) = match generate_access_token(&req.wallet_address, scope.clone(), &state.jwt_secret) {
        Ok(v) => v,
        Err(err) => return jwt_error_response(err).into_response(),
    };

    let (refresh_token, refresh_claims) = match generate_refresh_token(&req.wallet_address, scope, &state.jwt_secret) {
        Ok(v) => v,
        Err(err) => return jwt_error_response(err).into_response(),
    };

    if let (Some(cache), Some(jti)) = (&state.redis_cache, refresh_claims.jti.as_deref()) {
        let record = RefreshTokenRecord {
            wallet_address: req.wallet_address.clone(),
            issued_at: refresh_claims.iat,
            expires_at: refresh_claims.exp,
        };
        if let Err(err) = store_refresh_token(cache, jti, &record).await {
            return jwt_error_response(err).into_response();
        }
    }

    (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            refresh_token,
            token_type: "Bearer",
            expires_in: access_claims.exp - access_claims.iat,
        }),
    )
        .into_response()
}

pub async fn refresh_token(
    State(state): State<Arc<AuthHandlerState>>,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let claims = match validate_token(&req.refresh_token, &state.jwt_secret) {
        Ok(claims) if claims.token_type == TokenType::Refresh => claims,
        Ok(_) => return jwt_error_response(JwtError::InvalidToken).into_response(),
        Err(err) => return jwt_error_response(err).into_response(),
    };

    let scope = claims.scope.clone();
    let (access_token, access_claims) = match generate_access_token(&claims.sub, scope, &state.jwt_secret) {
        Ok(v) => v,
        Err(err) => return jwt_error_response(err).into_response(),
    };

    (
        StatusCode::OK,
        Json(json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": access_claims.exp - access_claims.iat,
        })),
    )
        .into_response()
}

pub async fn revoke_token(
    State(state): State<Arc<AuthHandlerState>>,
    Json(req): Json<RevokeTokenRequest>,
) -> impl IntoResponse {
    let Some(cache) = state.redis_cache.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Redis cache unavailable for token revocation" })),
        )
            .into_response();
    };

    if let Some(refresh_token) = req.refresh_token.as_deref() {
        let claims = match validate_token(refresh_token, &state.jwt_secret) {
            Ok(claims) => claims,
            Err(err) => return jwt_error_response(err).into_response(),
        };

        if let Some(jti) = claims.jti.as_deref() {
            if let Err(err) = revoke_refresh_token(cache, jti).await {
                return jwt_error_response(err).into_response();
            }
        }
    }

    if let Some(access_token) = req.access_token.as_deref() {
        let claims = match validate_token(access_token, &state.jwt_secret) {
            Ok(claims) => claims,
            Err(err) => return jwt_error_response(err).into_response(),
        };

        if let Some(jti) = claims.jti.as_deref() {
            let remaining = (claims.exp - chrono::Utc::now().timestamp()).max(0) as u64;
            if let Err(err) = blacklist_access_token(cache, jti, remaining).await {
                return jwt_error_response(err).into_response();
            }
        }
    }

    (StatusCode::OK, Json(json!({ "status": "revoked" }))).into_response()
}
