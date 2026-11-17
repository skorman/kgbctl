//! Request and response bodies of the `/api/v1/sync/*` endpoints, shared by
//! the server (`api::sync_routes`) and the client (`sync::remote`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::{Broker, ExecutionBatch, PositionUpdate, SyncStatus};

#[derive(Serialize, Deserialize, Debug)]
pub struct StartRun {
    pub broker: Broker,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Id {
    pub id: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FinishRun {
    /// `ok` or `error`; `running` is rejected.
    pub status: SyncStatus,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpsertAccount {
    pub broker: Broker,
    pub external_id: String,
    pub name: Option<String>,
    pub base_currency: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApplyPositions {
    pub account_id: i64,
    pub broker: Broker,
    pub updates: Vec<PositionUpdate>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LedgerCursor {
    /// Unix seconds of the newest stored entry.
    pub time: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Count {
    pub count: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LatestFill {
    pub executed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RecordExecutions {
    pub broker: Broker,
    pub batches: Vec<ExecutionBatch>,
}
