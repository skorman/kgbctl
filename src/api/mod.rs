//! Read-only HTTP API over the database, for frontends other than the TUI.
//! Serves what's already stored; it never contacts a broker.

mod auth;
mod dto;
mod error;
mod routes;

use std::io::IsTerminal;
use std::net::SocketAddr;

use anyhow::Context;
use axum::Router;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use axum::middleware;
use axum::routing::get;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::db::Db;

#[derive(Clone)]
pub struct AppState {
    db: Db,
    /// When set, every route but `/health` needs `Authorization: Bearer <token>`.
    token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    pub token: Option<String>,
    /// Allowed CORS origins; empty allows any.
    pub cors_origins: Vec<String>,
}

pub fn router(db: Db, token: Option<String>) -> Router {
    let state = AppState { db, token };

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
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
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
        .allow_methods([Method::GET])
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

    if config.token.is_none() {
        tracing::warn!("KGB_API_TOKEN is not set; the API is open to anyone who can reach it");
    }

    let app = router(db, config.token)
        .layer(cors(&config.cors_origins)?)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
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

    use super::*;
    use crate::db::{AssetClass, Broker, NewInstrument, PositionUpdate};
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
        let run = db.start_sync_run(broker).await.unwrap();
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

    async fn get(app: &Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut req = Request::get(uri);
        if let Some(t) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        let res = app
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn snapshot_matches_what_the_tui_loads() {
        let db = seeded().await;
        let expected = Snapshot::load(&db).await.unwrap();
        let app = router(db, None);

        let (status, body) = get(&app, "/api/v1/snapshot", None).await;
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
        let app = router(seeded().await, None);

        let (status, body) = get(&app, "/api/v1/tabs/crypto", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Crypto");
        assert_eq!(body["holdings"][0]["ticker"], "BTC");

        let (status, body) = get(&app, "/api/v1/tabs/nope", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");

        let (_, body) = get(&app, "/api/v1/tabs", None).await;
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
        let app = router(db, None);

        let (_, body) = get(&app, "/api/v1/positions?broker=kraken", None).await;
        let tickers: Vec<_> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|p| &p["ticker"])
            .collect();
        assert_eq!(tickers, ["BTC"]);

        let (_, body) = get(&app, "/api/v1/positions?account=U1", None).await;
        let nvda = body
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["ticker"] == "NVDA")
            .unwrap();
        let id = nvda["id"].as_i64().unwrap();

        let (status, body) = get(&app, &format!("/api/v1/positions/{id}/history"), None).await;
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
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "bad_request");

        let (status, _) = get(&app, "/api/v1/positions/9999", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = get(&app, "/api/v1/positions?broker=schwab", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn groups_include_totals_only_when_held() {
        let app = router(seeded().await, None);

        let (_, body) = get(&app, "/api/v1/groups", None).await;
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

        let (status, body) = get(&app, "/api/v1/groups/zeus", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["members"].as_array().unwrap().len(), 19);
    }

    #[tokio::test]
    async fn sync_runs_newest_first_and_limited() {
        let app = router(seeded().await, None);

        let (_, body) = get(&app, "/api/v1/sync/runs", None).await;
        let brokers: Vec<_> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["broker"])
            .collect();
        assert_eq!(brokers, ["kraken", "ibkr"]);

        let (_, body) = get(&app, "/api/v1/sync/runs?limit=1&broker=ibkr", None).await;
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["broker"], "ibkr");
    }

    #[tokio::test]
    async fn token_is_required_when_configured() {
        let app = router(seeded().await, Some("s3cret".into()));

        let (status, body) = get(&app, "/api/v1/snapshot", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
        assert_eq!(
            get(&app, "/api/v1/snapshot", Some("wrong")).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get(&app, "/api/v1/snapshot", Some("s3cret")).await.0,
            StatusCode::OK
        );

        let (status, body) = get(&app, "/api/v1/health", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["db"], "ok");
    }
}
