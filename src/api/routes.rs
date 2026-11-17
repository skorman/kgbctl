use std::collections::HashMap;

use axum::Json;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::AppState;
use super::dto::*;
use super::error::ApiError;
use crate::db::{Account, Broker, PositionSnapshot, PositionView, SyncRun};
use crate::portfolio::Snapshot;

type ApiResult<T> = Result<Json<T>, ApiError>;

const DEFAULT_RUN_LIMIT: i64 = 50;
const MAX_RUN_LIMIT: i64 = 500;

// Extractor rejections become our JSON error shape instead of axum's plain text.
fn query<T>(q: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    q.map(|Query(q)| q)
        .map_err(|e| ApiError::BadRequest(e.body_text()))
}

fn path<T>(p: Result<Path<T>, PathRejection>) -> Result<T, ApiError> {
    p.map(|Path(p)| p)
        .map_err(|e| ApiError::BadRequest(e.body_text()))
}

pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthDto>) {
    let version = env!("CARGO_PKG_VERSION");
    match state.db.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthDto {
                status: "ok",
                version,
                db: "ok",
            }),
        ),
        Err(e) => {
            tracing::error!("health check: {e:#}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthDto {
                    status: "degraded",
                    version,
                    db: "error",
                }),
            )
        }
    }
}

pub async fn snapshot(State(state): State<AppState>) -> ApiResult<SnapshotDto> {
    Ok(Json(Snapshot::load(&state.db).await?.into()))
}

pub async fn tabs(State(state): State<AppState>) -> ApiResult<Vec<TabSummaryDto>> {
    let snapshot = Snapshot::load(&state.db).await?;
    Ok(Json(snapshot.tabs.iter().map(Into::into).collect()))
}

pub async fn tab(
    State(state): State<AppState>,
    name: Result<Path<String>, PathRejection>,
) -> ApiResult<TabDto> {
    let name = path(name)?;
    let snapshot = Snapshot::load(&state.db).await?;
    snapshot
        .tabs
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(&name))
        .map(|t| Json(t.into()))
        .ok_or_else(|| ApiError::NotFound(format!("no tab named {name:?}")))
}

#[derive(Deserialize)]
pub struct PositionsQuery {
    broker: Option<Broker>,
    account: Option<String>,
}

pub async fn positions(
    State(state): State<AppState>,
    q: Result<Query<PositionsQuery>, QueryRejection>,
) -> ApiResult<Vec<PositionView>> {
    let q = query(q)?;
    let positions = state
        .db
        .list_positions()
        .await?
        .into_iter()
        .filter(|p| q.broker.is_none_or(|b| p.broker == b))
        .filter(|p| q.account.as_ref().is_none_or(|a| &p.account == a))
        .collect();
    Ok(Json(positions))
}

pub async fn position(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> ApiResult<PositionView> {
    let id = path(id)?;
    state
        .db
        .position(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("no open position {id}")))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

pub async fn position_history(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    q: Result<Query<HistoryQuery>, QueryRejection>,
) -> ApiResult<Vec<PositionSnapshot>> {
    let id = path(id)?;
    let q = query(q)?;
    Ok(Json(state.db.position_history(id, q.from, q.to).await?))
}

pub async fn accounts(State(state): State<AppState>) -> ApiResult<Vec<Account>> {
    Ok(Json(state.db.list_accounts().await?))
}

pub async fn groups(State(state): State<AppState>) -> ApiResult<Vec<GroupDto>> {
    // v_group_totals only has rows for groups with open positions.
    let mut totals: HashMap<_, _> = state
        .db
        .group_totals()
        .await?
        .into_iter()
        .map(|t| (t.name.to_lowercase(), t))
        .collect();
    let groups = state
        .db
        .list_groups()
        .await?
        .into_iter()
        .map(|g| GroupDto {
            totals: totals.remove(&g.name.to_lowercase()).map(Into::into),
            id: g.id,
            name: g.name,
            description: g.description,
        })
        .collect();
    Ok(Json(groups))
}

pub async fn group(
    State(state): State<AppState>,
    name: Result<Path<String>, PathRejection>,
) -> ApiResult<GroupDetailDto> {
    let name = path(name)?;
    let Some(group) = state.db.find_group(&name).await? else {
        return Err(ApiError::NotFound(format!("no group named {name:?}")));
    };
    let members = state.db.group_members(group.id).await?;
    Ok(Json(GroupDetailDto::new(group, members)))
}

pub async fn open_orders(State(state): State<AppState>) -> ApiResult<Vec<OrderDto>> {
    let orders = state.db.list_open_orders().await?;
    Ok(Json(orders.into_iter().map(Into::into).collect()))
}

#[derive(Deserialize)]
pub struct SyncRunsQuery {
    broker: Option<Broker>,
    limit: Option<i64>,
}

pub async fn sync_runs(
    State(state): State<AppState>,
    q: Result<Query<SyncRunsQuery>, QueryRejection>,
) -> ApiResult<Vec<SyncRun>> {
    let q = query(q)?;
    let limit = q.limit.unwrap_or(DEFAULT_RUN_LIMIT).clamp(1, MAX_RUN_LIMIT);
    Ok(Json(state.db.list_sync_runs(q.broker, limit).await?))
}
