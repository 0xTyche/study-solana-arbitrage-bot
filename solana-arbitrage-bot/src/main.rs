mod bot;
mod config;
mod constants;
mod dex;
mod kamino;
mod pools;
mod refresh;
mod transaction;

use clap::{App, Arg};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;


async fn main() -> anyhow::Result<()> {

    // 设置日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    info!("Starting Solana Arbitrage Bot");

    let matches = App::new("Solana Arbitrage Bot")
        .version("0.1.0")
        .author("Tyche")
        .about("A simplified Solana onchain arbitrage bot")
        .arg(
            Arg::with_name("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true)
                .default_value("config.toml"),
        )
        .get_matches();
}
