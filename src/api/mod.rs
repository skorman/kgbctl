//! HTTP API over the database, for frontends other than the TUI. It never
//! contacts a broker. Reads need any API user's token; the only writes are
//! token rotation and, for users with the sync permission, the `/sync/*`
//! endpoints a remote `kgbctl -c sync` pushes broker data through.

mod auth;
mod dto;
mod error;
mod routes;
mod sync_routes;
pub mod token;

use std::io::IsTerminal;
use std::net::SocketAddr;

use anyhow::Context;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use axum::middleware;
use axum::routing::{get, post};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::db::Db;

#[derive(Clone)]
pub struct AppState {
    db: Db,
}

#[derive(Clone, Debug)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    /// Allowed CORS origins; empty allows any.
    pub cors_origins: Vec<String>,
}

/// A Kraken ledger page or a batch of IBKR executions can be large.
const SYNC_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// Every route but `/health` needs `Authorization: Bearer <token>` for a user in
/// `api_users`; `/sync/*` also needs that user's `can_sync`.
pub fn router(db: Db) -> Router {
    let state = AppState { db };

    let sync = Router::new()
        .route("/sync/runs", post(sync_routes::start_run))
        .route("/sync/runs/{id}/finish", post(sync_routes::finish_run))
        .route(
            "/sync/runs/{id}/positions",
            post(sync_routes::apply_positions),
        )
        .route("/sync/accounts", post(sync_routes::upsert_account))
        .route(
            "/sync/accounts/{id}/kraken/cursor",
            get(sync_routes::kraken_cursor),
        )
        .route(
            "/sync/accounts/{id}/kraken/ledger",
            get(sync_routes::kraken_ledger).post(sync_routes::insert_kraken_ledger),
        )
        .route(
            "/sync/accounts/{id}/fills/latest",
            get(sync_routes::latest_fill),
        )
        .route(
            "/sync/accounts/{id}/executions",
            post(sync_routes::record_executions),
        )
        .route_layer(middleware::from_fn(auth::require_sync))
        .layer(DefaultBodyLimit::max(SYNC_BODY_LIMIT));

    let protected = Router::new()
        .route("/snapshot", get(routes::snapshot))
        .route("/tabs", get(routes::tabs))
        .route("/tabs/{name}", get(routes::tab))
        .route("/positions", get(routes::positions))
        .route("/positions/{id}", get(routes::position))
        .route("/positions/{id}/history", get(routes::position_history))
        .route("/accounts", get(routes::accounts))
        .route("/groups", get(routes::groups))
        .route("/groups/{name}", get(routes::group))
        .route("/orders/open", get(routes::open_orders))
        .route("/sync/runs", get(routes::sync_runs))
        .route("/token/rotate", post(routes::rotate_token))
        .merge(sync)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_user,
        ));

    let api = Router::new()
        .route("/health", get(routes::health))
        .merge(protected)
        .with_state(state);

    Router::new().nest("/api/v1", api)
}

fn cors(origins: &[String]) -> anyhow::Result<CorsLayer> {
    let allow = if origins.is_empty() {
        AllowOrigin::any()
    } else {
        let origins = origins
            .iter()
            .map(|o| HeaderValue::from_str(o).with_context(|| format!("CORS origin {o:?}")))
            .collect::<anyhow::Result<Vec<_>>>()?;
        AllowOrigin::list(origins)
    };
    Ok(CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(allow))
}

pub async fn serve(db: Db, config: ServeConfig) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kgbctl=info,tower_http=info".into()),
        )
        .init();

    if db.list_api_users().await?.is_empty() {
        tracing::warn!("no API users; add one with `kgbctl -c user-add --name <name>`");
    }

    let app = router(db).layer(cors(&config.cors_origins)?).layer(
        TraceLayer::new_for_http()
            // `user` is filled in by `auth::require_user` once the token checks out.
            .make_span_with(|req: &Request| {
                tracing::info_span!(
                    "request",
                    method = %req.method(),
                    uri = %req.uri(),
                    user = tracing::field::Empty,
                )
            })
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    );

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("binding {}", config.bind))?;
    tracing::info!("listening on http://{}/api/v1", config.bind);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Ctrl-C, or SIGTERM from systemd.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!("shutting down");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    use serde_json::json;

    use super::*;
    use crate::db::{AssetClass, Broker, NewInstrument, PositionUpdate, SyncStatus};
    use crate::portfolio::Snapshot;

    fn update(
        symbol: &str,
        class: AssetClass,
        qty: f64,
        price: f64,
        avg_cost: f64,
    ) -> PositionUpdate {
        PositionUpdate {
            instrument: NewInstrument {
                symbol: symbol.into(),
                asset_class: class,
                currency: "USD".into(),
                exchange: None,
                name: None,
            },
            broker_instrument_id: symbol.into(),
            quantity: qty,
            avg_cost: Some(avg_cost),
            cost_basis_usd: Some(qty * avg_cost),
            market_price: Some(price),
            market_value_usd: Some(qty * price),
            unrealized_pl_usd: Some(qty * (price - avg_cost)),
            unrealized_pl_pct: Some(price / avg_cost - 1.0),
            realized_pl_usd: None,
        }
    }

    async fn sync(db: &Db, broker: Broker, account: &str, updates: &[PositionUpdate]) {
        let account = db
            .upsert_account(broker, account, None, "USD")
            .await
            .unwrap();
        let run = db.start_sync_run(broker, None).await.unwrap();
        db.apply_position_sync(run, account, broker, updates)
            .await
            .unwrap();
        db.finish_sync_run_ok(run).await.unwrap();
    }

    async fn seeded() -> Db {
        use AssetClass::{Crypto, Stock};
        let db = Db::connect_in_memory().await.unwrap();
        sync(
            &db,
            Broker::Ibkr,
            "U1",
            &[
                update("NVDA", Stock, 10.0, 100.0, 80.0),
                update("SPY", Stock, 1.0, 500.0, 400.0),
            ],
        )
        .await;
        sync(
            &db,
            Broker::Kraken,
            "main",
            &[update("BTC", Crypto, 0.5, 90000.0, 60000.0)],
        )
        .await;
        db
    }

    /// A router over `db` with one read-only user, and that user's token.
    async fn authed(db: Db) -> (Router, String) {
        let token = token::generate().unwrap();
        db.create_api_user("Tester", &token::hash(&token), false)
            .await
            .unwrap();
        (router(db), token)
    }

    /// A token for a new user allowed to push syncs.
    async fn sync_user(db: &Db, name: &str) -> String {
        let token = token::generate().unwrap();
        db.create_api_user(name, &token::hash(&token), true)
            .await
            .unwrap();
        token
    }

    async fn get(app: &Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let (status, _, body) = send(app, Method::GET, uri, token).await;
        (status, body)
    }

    async fn send(
        app: &Router,
        method: Method,
        uri: &str,
        token: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        send_json(app, method, uri, token, None).await
    }

    async fn post(app: &Router, uri: &str, token: &str, body: Value) -> (StatusCode, Value) {
        let (status, _, body) = send_json(app, Method::POST, uri, Some(token), Some(body)).await;
        (status, body)
    }

    async fn send_json(
        app: &Router,
        method: Method,
        uri: &str,
        token: Option<&str>,
        json: Option<Value>,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        let body = match json {
            Some(v) => {
                req = req.header(CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, body)
    }

    #[tokio::test]
    async fn snapshot_matches_what_the_tui_loads() {
        let db = seeded().await;
        let expected = Snapshot::load(&db).await.unwrap();
        let (app, token) = authed(db).await;

        let (status, body) = get(&app, "/api/v1/snapshot", Some(&token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["totals"]["value"], expected.totals.value);
        assert_eq!(body["totals"]["basis"], expected.totals.basis);
        assert_eq!(body["sources"][0]["broker"], "ibkr");
        assert_eq!(body["sources"][0]["last_run"]["status"], "ok");
        assert_eq!(body["tabs"].as_array().unwrap().len(), expected.tabs.len());

        let prometheus = &body["tabs"][2];
        assert_eq!(prometheus["name"], "Prometheus");
        let nvda = &prometheus["holdings"][0];
        assert_eq!(nvda["ticker"], "NVDA");
        assert_eq!(nvda["cost"], 80.0);
        assert_eq!(nvda["pl"], 200.0);
        assert_eq!(nvda["pl_pct"], 0.25);
    }

    #[tokio::test]
    async fn tab_lookup_is_case_insensitive() {
        let (app, token) = authed(seeded().await).await;

        let (status, body) = get(&app, "/api/v1/tabs/crypto", Some(&token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Crypto");
        assert_eq!(body["holdings"][0]["ticker"], "BTC");

        let (status, body) = get(&app, "/api/v1/tabs/nope", Some(&token)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");

        let (_, body) = get(&app, "/api/v1/tabs", Some(&token)).await;
        assert_eq!(body[2]["name"], "Prometheus");
        assert_eq!(body[2]["holding_count"], 1);
    }

    #[tokio::test]
    async fn positions_filter_and_history() {
        let db = seeded().await;
        sync(
            &db,
            Broker::Ibkr,
            "U1",
            &[
                update("NVDA", AssetClass::Stock, 10.0, 120.0, 80.0),
                update("SPY", AssetClass::Stock, 1.0, 500.0, 400.0),
            ],
        )
        .await;
        let (app, token) = authed(db).await;

        let (_, body) = get(&app, "/api/v1/positions?broker=kraken", Some(&token)).await;
        let tickers: Vec<_> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|p| &p["ticker"])
            .collect();
        assert_eq!(tickers, ["BTC"]);

        let (_, body) = get(&app, "/api/v1/positions?account=U1", Some(&token)).await;
        let nvda = body
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["ticker"] == "NVDA")
            .unwrap();
        let id = nvda["id"].as_i64().unwrap();

        let (status, body) = get(
            &app,
            &format!("/api/v1/positions/{id}/history"),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let prices: Vec<_> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|s| &s["market_price"])
            .collect();
        assert_eq!(prices, [100.0, 120.0]);

        let (status, body) = get(
            &app,
            &format!("/api/v1/positions/{id}/history?from=yesterday"),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "bad_request");

        let (status, _) = get(&app, "/api/v1/positions/9999", Some(&token)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = get(&app, "/api/v1/positions?broker=schwab", Some(&token)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn groups_include_totals_only_when_held() {
        let (app, token) = authed(seeded().await).await;

        let (_, body) = get(&app, "/api/v1/groups", Some(&token)).await;
        let prometheus = body
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["name"] == "Prometheus")
            .unwrap();
        assert_eq!(prometheus["totals"]["value_usd"], 1000.0);
        let ares = body
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["name"] == "Ares")
            .unwrap();
        assert!(ares["totals"].is_null());

        let (status, body) = get(&app, "/api/v1/groups/zeus", Some(&token)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["members"].as_array().unwrap().len(), 19);
    }

    #[tokio::test]
    async fn sync_runs_newest_first_and_limited() {
        let (app, token) = authed(seeded().await).await;

        let (_, body) = get(&app, "/api/v1/sync/runs", Some(&token)).await;
        let brokers: Vec<_> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["broker"])
            .collect();
        assert_eq!(brokers, ["kraken", "ibkr"]);

        let (_, body) = get(&app, "/api/v1/sync/runs?limit=1&broker=ibkr", Some(&token)).await;
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["broker"], "ibkr");
    }

    #[tokio::test]
    async fn requests_need_a_known_users_token() {
        let db = seeded().await;
        let (app, token) = authed(db.clone()).await;

        let (status, body) = get(&app, "/api/v1/snapshot", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
        let wrong = token::generate().unwrap();
        assert_eq!(
            get(&app, "/api/v1/snapshot", Some(&wrong)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert!(
            db.list_api_users().await.unwrap()[0]
                .last_active_at
                .is_none()
        );

        assert_eq!(
            get(&app, "/api/v1/snapshot", Some(&token)).await.0,
            StatusCode::OK
        );
        let user = &db.list_api_users().await.unwrap()[0];
        assert_eq!(user.name, "Tester");
        assert!(user.last_active_at.is_some());

        let (status, body) = get(&app, "/api/v1/health", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["db"], "ok");
    }

    #[tokio::test]
    async fn rotating_returns_a_new_token_and_retires_the_old_one() {
        let (app, old) = authed(seeded().await).await;
        let rotate = "/api/v1/token/rotate";

        let (status, headers, body) = send(&app, Method::POST, rotate, Some(&old)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[axum::http::header::CACHE_CONTROL], "no-store");
        assert_eq!(body["name"], "Tester");
        let new = body["token"].as_str().unwrap().to_string();
        assert_ne!(new, old);

        assert_eq!(
            get(&app, "/api/v1/tabs", Some(&old)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get(&app, "/api/v1/tabs", Some(&new)).await.0,
            StatusCode::OK
        );

        // The retired token can't rotate either, and rotation needs a token at all.
        let (status, _, _) = send(&app, Method::POST, rotate, Some(&old)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = send(&app, Method::POST, rotate, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = send(&app, Method::GET, rotate, Some(&new)).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn sync_endpoints_need_the_sync_permission() {
        let db = seeded().await;
        let (app, reader) = authed(db.clone()).await;
        let pusher = sync_user(&db, "Laptop").await;
        let start = json!({ "broker": "kraken" });

        let (status, _, body) = send_json(
            &app,
            Method::POST,
            "/api/v1/sync/runs",
            None,
            Some(start.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");

        let (status, body) = post(&app, "/api/v1/sync/runs", &reader, start.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "forbidden");
        // A read-only user still reads.
        assert_eq!(
            get(&app, "/api/v1/tabs", Some(&reader)).await.0,
            StatusCode::OK
        );

        let (status, body) = post(&app, "/api/v1/sync/runs", &pusher, start).await;
        assert_eq!(status, StatusCode::OK);
        let run = db
            .sync_run(body["id"].as_i64().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.pushed_by.as_deref(), Some("Laptop"));
        assert_eq!(run.status, SyncStatus::Running);
    }

    #[tokio::test]
    async fn remote_sync_round_trip_stores_what_a_local_sync_would() {
        use AssetClass::Crypto;
        let db = Db::connect_in_memory().await.unwrap();
        let pusher = sync_user(&db, "Laptop").await;
        let app = router(db.clone());
        // A stale run from a client that died.
        let stale = db
            .start_sync_run(Broker::Kraken, Some("Laptop"))
            .await
            .unwrap();

        let (status, body) = post(
            &app,
            "/api/v1/sync/runs",
            &pusher,
            json!({"broker": "kraken"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let run = body["id"].as_i64().unwrap();
        assert_eq!(
            db.sync_run(stale).await.unwrap().unwrap().status,
            SyncStatus::Error
        );

        let (status, body) = post(
            &app,
            "/api/v1/sync/accounts",
            &pusher,
            json!({"broker": "kraken", "external_id": "main", "name": null, "base_currency": "USD"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let account = body["id"].as_i64().unwrap();

        let (status, body) = get(
            &app,
            &format!("/api/v1/sync/accounts/{account}/kraken/cursor"),
            Some(&pusher),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["time"].is_null());

        let entries = json!([
            {"ledger_id": "L1", "refid": "T1", "time": 100.0, "entry_type": "trade", "subtype": null,
             "asset": "ZUSD", "amount": -100.0, "fee": 0.0, "balance": null, "raw": "{}"},
            {"ledger_id": "L2", "refid": "T1", "time": 100.0, "entry_type": "trade", "subtype": null,
             "asset": "XXBT", "amount": 1.0, "fee": 0.0, "balance": null, "raw": "{}"}
        ]);
        let ledger_uri = format!("/api/v1/sync/accounts/{account}/kraken/ledger");
        let (status, body) = post(&app, &ledger_uri, &pusher, entries.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 2);
        let (_, body) = post(&app, &ledger_uri, &pusher, entries).await;
        assert_eq!(body["count"], 0);
        let (_, body) = get(
            &app,
            &format!("/api/v1/sync/accounts/{account}/kraken/cursor"),
            Some(&pusher),
        )
        .await;
        assert_eq!(body["time"], 100.0);
        let (_, body) = get(&app, &ledger_uri, Some(&pusher)).await;
        assert_eq!(body.as_array().unwrap().len(), 2);
        assert!(body[0]["raw"].is_null(), "raw is not sent back");

        let updates = serde_json::to_value([update("BTC", Crypto, 1.0, 90000.0, 100.0)]).unwrap();
        let (status, _) = post(
            &app,
            &format!("/api/v1/sync/runs/{run}/positions"),
            &pusher,
            json!({"account_id": account, "broker": "kraken", "updates": updates}),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // Broker mismatch between body and run/account is refused.
        let (status, _) = post(
            &app,
            &format!("/api/v1/sync/runs/{run}/positions"),
            &pusher,
            json!({"account_id": account, "broker": "ibkr", "updates": []}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = post(
            &app,
            &format!("/api/v1/sync/runs/{run}/finish"),
            &pusher,
            json!({"status": "ok", "error": null}),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // Finished runs can't be written to again.
        let (status, _) = post(
            &app,
            &format!("/api/v1/sync/runs/{run}/finish"),
            &pusher,
            json!({"status": "ok", "error": null}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = post(
            &app,
            "/api/v1/sync/runs/9999/finish",
            &pusher,
            json!({"status": "ok", "error": null}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // What a local sync of the same data produces.
        let local = Db::connect_in_memory().await.unwrap();
        sync(
            &local,
            Broker::Kraken,
            "main",
            &[update("BTC", Crypto, 1.0, 90000.0, 100.0)],
        )
        .await;
        let (ours, theirs) = (
            Snapshot::load(&db).await.unwrap(),
            Snapshot::load(&local).await.unwrap(),
        );
        assert_eq!(ours.totals, theirs.totals);
        assert_eq!(ours.tabs, theirs.tabs);
        let run = db.last_sync_run(Broker::Kraken).await.unwrap().unwrap();
        assert_eq!(run.status, SyncStatus::Ok);
        assert_eq!(run.pushed_by.as_deref(), Some("Laptop"));
    }
}
