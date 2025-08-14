use crate::{
    client::{doublezero::DzRpcClient, solana::SolRpcClient},
    verify_access_request, Result,
};
use doublezero_passport::instruction::AccessMode;
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
                _ = backfill_timer.tick() => {
                    let access_modes = self.sol_rpc_client.gets_access_mode().await?;

                    info!(count = access_modes.len(), "processing unhandled access requests");

                    for access_mode in access_modes {
                        if verify_access_request(&access_mode).is_ok() {
                            let AccessMode::SolanaValidator {
                                service_key,
                                ..
                            } = access_mode;
                            self.dz_rpc_client.fund_authorized_user(&service_key, self.onboarding_lamports).await?;
                        }

                    }
                }
                event = self.rx.recv() => {
                    if let Some(signature) = event {
                        info!(%signature, "received access request txn");
                        self.handle_access_request(signature).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_access_request(&self, signature: Signature) -> Result<()> {
        let access_mode = self
            .sol_rpc_client
            .get_access_mode_from_signature(signature)
            .await?;

        if verify_access_request(&access_mode).is_ok() {
            let AccessMode::SolanaValidator { service_key, .. } = access_mode;
            self.dz_rpc_client
                .fund_authorized_user(&service_key, self.onboarding_lamports)
                .await?;
        }

        Ok(())
    }
}
