#!/bin/bash

set -eu

### First build the Revenue Distribution program.
cargo build-sbf -- \
    -p doublezero-revenue-distribution \
    --features development,entrypoint

### Upgrade Revenue Distribution program.
solana program deploy \
    -u l \
    --program-id dzrevZC94tBLwuHw1dyynZxaXTWyp7yocsinyEVPtt4 \
    target/deploy/doublezero_revenue_distribution.so

CLI_BIN=target/release/2z

echo "Waiting 15 seconds for program upgrade to finalize"
sleep 15

### Execute `admin migrate-program-accounts`.
# $CLI_BIN admin migrate-program-accounts -u l -v
