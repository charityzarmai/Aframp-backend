//! JWT-based authentication module for Aframp API.
//!
//! Provides:
//! - `jwt`        – token generation, validation, Redis storage helpers
//! - `middleware` – Axum middleware (`require_auth`, `require_admin`)
//! - `handlers`   – HTTP handlers for /api/auth/{token,refresh,revoke}
//! - `routes`     – Router builder

#[cfg(feature = "database")]
pub mod handlers;
#[cfg(feature = "database")]
pub mod jwt;
#[cfg(feature = "database")]
pub mod middleware;

#[cfg(feature = "database")]
pub use handlers::AuthHandlerState;
#[cfg(feature = "database")]
pub use jwt::{JwtConfig, JwtError, Scope, TokenClaims, TokenType};
#[cfg(feature = "database")]
pub use middleware::AuthState;

#[cfg(feature = "database")]
use axum::{routing::post, Router};
#[cfg(feature = "database")]
use std::sync::Arc;

/// Build the auth router and mount it at `/api/auth`.
#[cfg(feature = "database")]
pub fn auth_router(state: Arc<AuthHandlerState>) -> Router {
    Router::new()
        .route("/api/auth/token", post(handlers::generate_token))
        .route("/api/auth/refresh", post(handlers::refresh_token))
        .route("/api/auth/revoke", post(handlers::revoke_token))
        .with_state(state)
}
