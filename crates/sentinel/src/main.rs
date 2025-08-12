use clap::Parser;
use doublezero_ledger_sentinel::settings::{AppArgs, Settings};
use metrics_exporter_prometheus::PrometheusBuilder;
use solana_sdk::signer::Signer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = AppArgs::parse();
    let settings = Settings::new(args.config)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&settings.log))
        .with(tracing_subscriber::fmt::layer())
        .init();

    PrometheusBuilder::new()
        .with_http_listener(settings.metrics_addr())
        .install()?;

    let sol_rpc = settings.sol_rpc();
    let sol_ws = settings.sol_ws();
    let dz_rpc = settings.dz_rpc();
    let keypair = settings.keypair();

    info!(
        %sol_rpc,
        %sol_ws,
        %dz_rpc,
        pubkey = %keypair.pubkey(),
        "DoubleZero Ledger Sentinel starting"
    );

    Ok(())
}
