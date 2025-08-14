use crate::{
    client::{doublezero::DzRpcClient, solana::SolRpcClient},
    Result,
};
use solana_sdk::signature::{Keypair, Signature};
use std::{sync::Arc, time::Duration};
use tokio::{sync::mpsc::UnboundedReceiver, time::interval};
use tokio_util::sync::CancellationToken;
use tracing::info;
use url::Url;

const BACKFILL_TIMER: Duration = Duration::from_secs(60 * 60);

pub struct Sentinel {
    dz_rpc_client: DzRpcClient,
    sol_rpc_client: SolRpcClient,
    rx: UnboundedReceiver<Signature>,
    onboarding_lamports: u64,
}

impl Sentinel {
    pub async fn new(
        dz_rpc: Url,
        sol_rpc: Url,
        keypair: Arc<Keypair>,
        rx: UnboundedReceiver<Signature>,
        onboarding_lamports: u64,
    ) -> Result<Self> {
        Ok(Self {
            dz_rpc_client: DzRpcClient::new(dz_rpc, keypair.clone()),
            sol_rpc_client: SolRpcClient::new(sol_rpc, keypair),
            rx,
            onboarding_lamports,
        })
    }

    pub async fn run(&mut self, shutdown_listener: CancellationToken) -> Result<()> {
        let mut backfill_timer = interval(BACKFILL_TIMER);

        loop {
            tokio::select! {
                biased;
                _ = shutdown_listener.cancelled() => break,
                // run the backfill check on a ticker
                _ = backfill_timer.tick() => {
                    info!("fetching unhandled access requests");
                }
                // handle messages from the websocket handler
                event = self.rx.recv() => {
                    if let Some(signature) = event {
                        info!(%signature, "received access request txn");
                    }
                }
            }
        }

        Ok(())
    }
}
