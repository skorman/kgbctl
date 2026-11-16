// Most of the adapter isn't wired to commands yet.
#[allow(dead_code)]
mod db;
mod sync;
mod tui;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use db::{Broker, Db};
use sync::{Config, SyncOptions, ibkr, kraken};

#[derive(ValueEnum, Clone, Debug, PartialEq)]
enum Command {
    Quote,
    History,
    Watchlist,
    Returns,
    Sync,
    Tui,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum BrokerArg {
    Ibkr,
    Kraken,
}

impl From<BrokerArg> for Broker {
    fn from(b: BrokerArg) -> Self {
        match b {
            BrokerArg::Ibkr => Broker::Ibkr,
            BrokerArg::Kraken => Broker::Kraken,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    // Command to run
    #[arg(short, long, value_enum)]
    command: Command,

    // Turn on verbose logging
    #[arg(short, long)]
    verbose: bool,

    // Path to the SQLite database
    #[arg(long, env = "KGB_DB", default_value = "var/kgb.db")]
    db: PathBuf,

    // Sync only this broker (default: all)
    #[arg(long, value_enum)]
    broker: Option<BrokerArg>,

    // Ignore stored sync cursors and refetch all available history
    #[arg(long)]
    full: bool,

    #[arg(long, env = "KRAKEN_API_KEY", hide_env_values = true)]
    kraken_api_key: Option<String>,

    #[arg(long, env = "KRAKEN_SECRET_KEY", hide_env_values = true)]
    kraken_secret_key: Option<String>,

    // TWS / IB Gateway socket API
    #[arg(long, env = "IBKR_HOST", default_value = "127.0.0.1")]
    ibkr_host: String,

    #[arg(long, env = "IBKR_PORT", default_value_t = 7496)]
    ibkr_port: u16,

    #[arg(long, env = "IBKR_CLIENT_ID", default_value_t = 19)]
    ibkr_client_id: i32,
}

fn sync_config(args: &CliArgs) -> Config {
    Config {
        kraken: args
            .kraken_api_key
            .clone()
            .zip(args.kraken_secret_key.clone())
            .map(|(api_key, secret)| kraken::Config { api_key, secret }),
        ibkr: ibkr::Config {
            host: args.ibkr_host.clone(),
            port: args.ibkr_port,
            client_id: args.ibkr_client_id,
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    let db = Db::connect(&args.db).await?;
    db.migrate().await?;

    match args.command {
        Command::Quote => println!("Creating quote!"),
        Command::History => println!("Obtaining history!"),
        Command::Watchlist => {
            for group in db.list_groups().await? {
                let members = db.group_members(group.id).await?;
                let symbols: Vec<_> = members.iter().map(|i| i.symbol.as_str()).collect();
                println!("{:<12} {}", group.name, symbols.join(" "));
            }
        }
        Command::Returns => println!("Managing returns!"),
        Command::Sync => {
            let config = sync_config(&args);
            let brokers = match args.broker {
                Some(b) => vec![b.into()],
                None => vec![Broker::Ibkr, Broker::Kraken],
            };
            let opts = SyncOptions { full: args.full };

            let mut failed = false;
            for (broker, result) in sync::run_all(&db, &brokers, &config, opts).await {
                match result {
                    Ok(r) => println!(
                        "{broker:?}: {} positions, {} new {}",
                        r.positions, r.new_records, r.record_kind
                    ),
                    Err(e) => {
                        failed = true;
                        eprintln!("{broker:?}: sync failed: {e:#}");
                    }
                }
            }
            if failed {
                anyhow::bail!("one or more syncs failed");
            }
        }
        Command::Tui => tui::run(db, sync_config(&args)).await?,
    }

    Ok(())
}
