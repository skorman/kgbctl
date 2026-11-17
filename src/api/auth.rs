use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use super::error::ApiError;
use super::{AppState, token};

/// The authenticated user, available to handlers as `Extension<Caller>`.
#[derive(Clone, Debug)]
pub struct Caller {
    pub id: i64,
    pub name: String,
    /// Hash of the token this request was made with.
    pub token_hash: String,
}

/// Require `Authorization: Bearer <token>` matching an `api_users` row. Records
/// the user on the request's log span and as their latest activity.
pub async fn require_user(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(hash) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(token::hash)
    else {
        return Err(ApiError::Unauthorized);
    };
    // Lookup is by hash, so there's no plaintext comparison to time.
    let Some(user) = state.db.authenticate_api_user(&hash).await? else {
        tracing::warn!("rejected request with an unknown token");
        return Err(ApiError::Unauthorized);
    };
    tracing::Span::current().record("user", user.name.as_str());
    req.extensions_mut().insert(Caller {
        id: user.id,
        name: user.name,
        token_hash: hash,
    });
    Ok(next.run(req).await)
}
