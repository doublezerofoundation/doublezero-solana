use crate::{
    calculator::{
        csv_exporter,
        data_prep::PreparedData,
        input::RewardInput,
        keypair_loader::load_keypair,
        ledger_operations::{self, write_and_track, WriteSummary},
        proof::{
            ContributorRewardProof, ContributorRewardsMerkleRoot, ContributorRewardsMerkleTree,
        },
        shapley_aggregator::aggregate_shapley_outputs,
        util::print_demands,
    },
    ingestor::fetcher::Fetcher,
    settings::Settings,
};
use anyhow::Result;
use network_shapley::{shapley::ShapleyInput, types::Demand};
use rayon::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, path::PathBuf, time::Instant};
use tabled::{builder::Builder as TableBuilder, settings::Style};
use tracing::info;

#[derive(Debug)]
pub struct Orchestrator {
    settings: Settings,
}

impl Orchestrator {
    pub fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }

    pub async fn calculate_rewards(
        &self,
        epoch: Option<u64>,
        output_dir: Option<PathBuf>,
        keypair_path: Option<PathBuf>,
        dry_run: bool,
    ) -> Result<()> {
        let fetcher = Fetcher::from_settings(&self.settings)?;

        // Prepare all data
        let prep_data = PreparedData::new(&fetcher, epoch).await?;
        let fetch_epoch = prep_data.epoch;
        let device_telemetry = prep_data.device_telemetry;
        let internet_telemetry = prep_data.internet_telemetry;
        let shapley_inputs = prep_data.shapley_inputs;

        let input_config = RewardInput::new(
            fetch_epoch,
            self.settings.shapley.clone(),
            &shapley_inputs,
            &borsh::to_vec(&device_telemetry)?,
            &borsh::to_vec(&internet_telemetry)?,
        );

        // Optionally write CSVs
        if let Some(ref output_dir) = output_dir {
            info!("Writing CSV files to {}", output_dir.display());
            csv_exporter::export_to_csv(
                output_dir,
                &shapley_inputs.devices,
                &shapley_inputs.private_links,
                &shapley_inputs.public_links,
                &shapley_inputs.city_stats,
            )?;
            info!("Exported CSV files successfully!");
        }

        // Group demands by start city
        let mut demands_by_city: HashMap<String, Vec<Demand>> = HashMap::new();
        for demand in shapley_inputs.demands.clone() {
            demands_by_city
                .entry(demand.start.clone())
                .or_default()
                .push(demand);
        }
        let demand_groups: Vec<(String, Vec<Demand>)> = demands_by_city.into_iter().collect();

        // Collect per-city Shapley outputs in parallel
        let start_time = Instant::now();
        let per_city_shapley_outputs: HashMap<String, Vec<(String, f64)>> = demand_groups
            .par_iter()
            .map(|(city, demands)| {
                info!(
                    "City: {city}, Demand: \n{}",
                    print_demands(demands, 1_000_000)
                );

                // Optionally write demands per city
                if let Some(ref output_dir) = output_dir {
                    csv_exporter::write_demands_csv(output_dir, city, demands)
                        .expect("Failed to write demands CSV");
                }

                // Build shapley inputs
                let input = ShapleyInput {
                    private_links: shapley_inputs.private_links.clone(),
                    devices: shapley_inputs.devices.clone(),
                    demands: demands.clone(),
                    public_links: shapley_inputs.public_links.clone(),
                    operator_uptime: self.settings.shapley.operator_uptime,
                    contiguity_bonus: self.settings.shapley.contiguity_bonus,
                    demand_multiplier: self.settings.shapley.demand_multiplier,
                };

                // Shapley output
                let output = input.compute().expect("Failed to compute Shapley values");

                // Print per-city table
                let table = TableBuilder::from(output.clone())
                    .build()
                    .with(Style::psql().remove_horizontals())
                    .to_string();
                info!("Shapley Output for {city}:\n{}", table);

                // Store raw values for aggregation
                let city_values: Vec<(String, f64)> = output
                    .into_iter()
                    .map(|(operator, shapley_value)| (operator, shapley_value.value))
                    .collect();

                (city.clone(), city_values)
            })
            .collect();

        let elapsed = start_time.elapsed();
        info!(
            "Shapley computation completed in {:.2?} for {} cities",
            elapsed,
            per_city_shapley_outputs.len()
        );

        // Aggregate consolidated Shapley output
        if !per_city_shapley_outputs.is_empty() {
            let shapley_output =
                aggregate_shapley_outputs(&per_city_shapley_outputs, &shapley_inputs.city_weights)?;

            // Print shapley_output table
            let mut table_builder = TableBuilder::default();
            table_builder.push_record(["Operator", "Value", "Proportion (%)"]);

            for (operator, val) in shapley_output.iter() {
                table_builder.push_record([
                    operator,
                    &val.value.to_string(),
                    &format!("{:.2}", val.proportion * 100.0),
                ]);
            }

            let table = table_builder
                .build()
                .with(Style::psql().remove_horizontals())
                .to_string();
            info!("Shapley Output:\n{}", table);

            // Write shapley output CSV if output directory is specified
            if let Some(ref output_dir) = output_dir {
                csv_exporter::write_consolidated_shapley_csv(output_dir, &shapley_output)?;
            }

            // Construct merkle tree
            let merkle_tree = ContributorRewardsMerkleTree::new(fetch_epoch, &shapley_output)?;
            let merkle_root = merkle_tree.compute_root()?;
            info!("merkle_root: {:#?}", merkle_root);

            // Perform batch writes to ledger
            if !dry_run {
                let payer_signer = load_keypair(&keypair_path)?;
                let mut summary = WriteSummary::default();

                // Write device telemetry
                let device_prefix = self.settings.prefixes.device_telemetry.as_bytes();
                write_and_track(
                    &fetcher.rpc_client,
                    &payer_signer,
                    &[device_prefix, &fetch_epoch.to_le_bytes()],
                    &device_telemetry,
                    "device telemetry aggregates",
                    &mut summary,
                    self.settings.rpc.rps_limit,
                )
                .await;

                // Write internet telemetry
                let internet_prefix = self.settings.prefixes.internet_telemetry.as_bytes();
                write_and_track(
                    &fetcher.rpc_client,
                    &payer_signer,
                    &[internet_prefix, &fetch_epoch.to_le_bytes()],
                    &internet_telemetry,
                    "internet telemetry aggregates",
                    &mut summary,
                    self.settings.rpc.rps_limit,
                )
                .await;

                // Write reward input
                let reward_prefix = self.settings.prefixes.reward_input.as_bytes();
                write_and_track(
                    &fetcher.rpc_client,
                    &payer_signer,
                    &[reward_prefix, &fetch_epoch.to_le_bytes()],
                    &input_config,
                    "reward calculation input",
                    &mut summary,
                    self.settings.rpc.rps_limit,
                )
                .await;

                // Write merkle root
                let contributor_prefix = self.settings.prefixes.contributor_rewards.as_bytes();
                let merkle_root_data = ContributorRewardsMerkleRoot {
                    epoch: fetch_epoch,
                    root: merkle_root,
                    total_contributors: merkle_tree.len() as u32,
                };
                write_and_track(
                    &fetcher.rpc_client,
                    &payer_signer,
                    &[contributor_prefix, &fetch_epoch.to_le_bytes()],
                    &merkle_root_data,
                    "contributor rewards merkle root",
                    &mut summary,
                    self.settings.rpc.rps_limit,
                )
                .await;

                // Write contributor proofs
                for (index, reward) in merkle_tree.rewards().iter().enumerate() {
                    let proof = merkle_tree.generate_proof(index)?;
                    let proof_bytes = borsh::to_vec(&proof)?;

                    let proof_data = ContributorRewardProof {
                        epoch: fetch_epoch,
                        contributor: reward.operator.clone(),
                        reward: reward.clone(),
                        proof_bytes,
                        index: index as u32,
                    };

                    write_and_track(
                        &fetcher.rpc_client,
                        &payer_signer,
                        &[
                            contributor_prefix,
                            &fetch_epoch.to_le_bytes(),
                            reward.operator.as_bytes(),
                        ],
                        &proof_data,
                        &format!("proof for contributor {}", reward.operator),
                        &mut summary,
                        self.settings.rpc.rps_limit,
                    )
                    .await;
                }

                // Log final summary
                info!("{}", summary);

                // Return error if not all successful
                if !summary.all_successful() {
                    anyhow::bail!(
                        "Some writes failed: {}/{} successful",
                        summary.successful_count(),
                        summary.total_count()
                    );
                }
            } else {
                info!(
                    "DRY-RUN: Would perform batch writes for epoch {}",
                    fetch_epoch
                );
                info!(
                    "  - Device telemetry: {} bytes",
                    borsh::to_vec(&device_telemetry)?.len()
                );
                info!(
                    "  - Internet telemetry: {} bytes",
                    borsh::to_vec(&internet_telemetry)?.len()
                );
                info!(
                    "  - Reward input: {} bytes",
                    borsh::to_vec(&input_config)?.len()
                );
                info!(
                    "  - Merkle root: {} bytes",
                    borsh::to_vec(&ContributorRewardsMerkleRoot {
                        epoch: fetch_epoch,
                        root: merkle_root,
                        total_contributors: merkle_tree.len() as u32,
                    })?
                    .len()
                );
                info!("  - {} contributor proofs", merkle_tree.len());
            }
        }

        Ok(())
    }

    pub async fn read_telemetry_aggregates(&self, epoch: u64, payer_pubkey: &Pubkey) -> Result<()> {
        ledger_operations::read_telemetry_aggregates(&self.settings, epoch, payer_pubkey).await
    }

    pub async fn check_contributor_reward(
        &self,
        contributor: &str,
        epoch: u64,
        payer_pubkey: &Pubkey,
    ) -> Result<()> {
        ledger_operations::check_contributor_reward(
            &self.settings,
            contributor,
            epoch,
            payer_pubkey,
        )
        .await
    }

    pub async fn close_record(
        &self,
        record_type: &str,
        epoch: u64,
        keypair_path: Option<PathBuf>,
        contributor: Option<String>,
    ) -> Result<()> {
        ledger_operations::close_record(
            &self.settings,
            record_type,
            epoch,
            keypair_path,
            contributor,
        )
        .await
    }

    pub async fn read_reward_input(&self, epoch: u64, payer_pubkey: &Pubkey) -> Result<()> {
        ledger_operations::read_reward_input(&self.settings, epoch, payer_pubkey).await
    }
}
