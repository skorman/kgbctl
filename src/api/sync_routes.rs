//! Write endpoints used by a remote `kgbctl -c sync`: the local machine talks
//! to the brokers and pushes what a sync would store here. Each handler wraps
//! the `Db` method a local sync would call, so both paths store the same thing.

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};

use super::AppState;
use super::auth::Caller;
use super::error::ApiError;
use super::routes::path;
use crate::db::{Broker, KrakenLedgerEntry, SyncStatus};
use crate::sync::wire::*;

type ApiResult<T> = Result<Json<T>, ApiError>;

/// Start a run; any run of the same broker still marked running is failed
/// first, since a client that died can't finish it.
pub async fn start_run(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    Json(body): Json<StartRun>,
) -> ApiResult<Id> {
    let superseded = state.db.supersede_running_syncs(body.broker).await?;
    if superseded > 0 {
        tracing::warn!(broker = ?body.broker, superseded, "closed stale running sync runs");
    }
    let id = state
        .db
        .start_sync_run(body.broker, Some(&caller.name))
        .await?;
    tracing::info!(run = id, broker = ?body.broker, "remote sync started");
    Ok(Json(Id { id }))
}

pub async fn finish_run(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    Json(body): Json<FinishRun>,
) -> Result<StatusCode, ApiError> {
    let id = path(id)?;
    running_run(&state, id).await?;
    match body.status {
        SyncStatus::Ok => state.db.finish_sync_run_ok(id).await?,
        SyncStatus::Error => {
            let error = body.error.as_deref().unwrap_or("unknown error");
            state.db.finish_sync_run_err(id, error).await?
        }
        SyncStatus::Running => {
            return Err(ApiError::BadRequest("status must be ok or error".into()));
        }
    }
    tracing::info!(run = id, status = ?body.status, "remote sync finished");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn upsert_account(
    State(state): State<AppState>,
    Json(body): Json<UpsertAccount>,
) -> ApiResult<Id> {
    if body.external_id.trim().is_empty() {
        return Err(ApiError::BadRequest("external_id is required".into()));
    }
    let id = state
        .db
        .upsert_account(
            body.broker,
            &body.external_id,
            body.name.as_deref(),
            &body.base_currency,
        )
        .await?;
    Ok(Json(Id { id }))
}

pub async fn apply_positions(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    Json(body): Json<ApplyPositions>,
) -> Result<StatusCode, ApiError> {
    let run_id = path(id)?;
    let run_broker = running_run(&state, run_id).await?;
    let account_broker = account_broker(&state, body.account_id).await?;
    if body.broker != run_broker || body.broker != account_broker {
        return Err(ApiError::BadRequest(
            "broker does not match the run and account".into(),
        ));
    }
    state
        .db
        .apply_position_sync(run_id, body.account_id, body.broker, &body.updates)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn kraken_cursor(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> ApiResult<LedgerCursor> {
    let account_id = kraken_account(&state, id).await?;
    let time = state.db.kraken_ledger_cursor(account_id).await?;
    Ok(Json(LedgerCursor { time }))
}

pub async fn insert_kraken_ledger(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    Json(entries): Json<Vec<KrakenLedgerEntry>>,
) -> ApiResult<Count> {
    let account_id = kraken_account(&state, id).await?;
    let count = state.db.insert_kraken_ledger(account_id, &entries).await?;
    Ok(Json(Count { count }))
}

/// The stored ledger without `raw`, which cost-basis replay doesn't need.
pub async fn kraken_ledger(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> ApiResult<Vec<KrakenLedgerEntry>> {
    let account_id = kraken_account(&state, id).await?;
    let mut ledger = state.db.kraken_ledger(account_id).await?;
    for e in &mut ledger {
        e.raw = None;
    }
    Ok(Json(ledger))
}

pub async fn latest_fill(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> ApiResult<LatestFill> {
    let account_id = path(id)?;
    account_broker(&state, account_id).await?;
    let executed_at = state.db.latest_fill_time(account_id).await?;
    Ok(Json(LatestFill { executed_at }))
}

pub async fn record_executions(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    Json(body): Json<RecordExecutions>,
) -> ApiResult<Count> {
    let account_id = path(id)?;
    if account_broker(&state, account_id).await? != body.broker {
        return Err(ApiError::BadRequest(
            "broker does not match the account".into(),
        ));
    }
    let count = state
        .db
        .record_executions(account_id, body.broker, &body.batches)
        .await?;
    Ok(Json(Count { count }))
}

/// The broker of run `id`, which must exist and still be running.
async fn running_run(state: &AppState, id: i64) -> Result<Broker, ApiError> {
    let run = state
        .db
        .sync_run(id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no sync run {id}")))?;
    if run.status != SyncStatus::Running {
        return Err(ApiError::BadRequest(format!(
            "sync run {id} already finished"
        )));
    }
    Ok(run.broker)
}

/// The broker of account `id`, which must exist.
async fn account_broker(state: &AppState, id: i64) -> Result<Broker, ApiError> {
    state
        .db
        .account(id)
        .await?
        .map(|a| a.broker)
        .ok_or_else(|| ApiError::NotFound(format!("no account {id}")))
}

async fn kraken_account(
    state: &AppState,
    id: Result<Path<i64>, PathRejection>,
) -> Result<i64, ApiError> {
    let id = path(id)?;
    if account_broker(state, id).await? != Broker::Kraken {
        return Err(ApiError::BadRequest(format!(
            "account {id} is not a Kraken account"
        )));
    }
    Ok(id)
}
