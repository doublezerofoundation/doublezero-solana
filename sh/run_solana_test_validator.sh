#!/bin/bash

set -eu

ROOT_DIR=$(cd "$(dirname "$0")/.."; pwd)

LOCALNET_DIR=$ROOT_DIR/localnet
LOCALNET_CACHE_DIR=$LOCALNET_DIR/cache

mkdir -p $LOCALNET_CACHE_DIR

### Dump program accounts from Solana devnet into test-ledger
solana program dump -u d dzrevZC94tBLwuHw1dyynZxaXTWyp7yocsinyEVPtt4 $LOCALNET_CACHE_DIR/dzrevZC94tBLwuHw1dyynZxaXTWyp7yocsinyEVPtt4.so
solana program dump -u d dzpt2dM8g9qsLxpdddnVvKfjkCLVXd82jrrQVJigCPV $LOCALNET_CACHE_DIR/dzpt2dM8g9qsLxpdddnVvKfjkCLVXd82jrrQVJigCPV.so

DEFAULT_USER_KEYPAIR=$HOME/.config/solana/id.json

if [ ! -f $DEFAULT_USER_KEYPAIR ]; then
    echo "Generating user keypair"
    solana-keygen new --silent --no-bip39-passphrase
fi

USER_KEY=$(solana address)

### Run a validator with the test-ledger
solana-test-validator -u d \
    --reset \
    --upgradeable-program \
    dzrevZC94tBLwuHw1dyynZxaXTWyp7yocsinyEVPtt4 \
    $LOCALNET_CACHE_DIR/dzrevZC94tBLwuHw1dyynZxaXTWyp7yocsinyEVPtt4.so \
    $USER_KEY \
    --upgradeable-program \
    dzpt2dM8g9qsLxpdddnVvKfjkCLVXd82jrrQVJigCPV \
    $LOCALNET_CACHE_DIR/dzpt2dM8g9qsLxpdddnVvKfjkCLVXd82jrrQVJigCPV.so \
    $USER_KEY \
    --clone \
    devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj

