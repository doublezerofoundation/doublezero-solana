#!/bin/bash

set -eu

CLI_BIN=target/release/2z

$CLI_BIN -h
echo

### Establish another payer.

echo "solana-keygen new --silent --no-bip39-passphrase -o another_payer.json"
solana-keygen new --silent --no-bip39-passphrase -o another_payer.json
solana airdrop -u l 69 -k another_payer.json
echo

### Admin commands.

$CLI_BIN admin -h
echo

echo "admin initialize -u l -v --program passport"
$CLI_BIN admin initialize -u l -v --program passport
echo

echo "admin initialize -u l -v --program revenue-distribution"
$CLI_BIN admin initialize -u l -v --program revenue-distribution
echo

### Set admin to bogus address.
echo "admin set-admin -u l -v --program passport devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj"
$CLI_BIN admin set-admin \
    -u l \
    -v \
    --program passport \
    devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj
echo

### Set admin to upgrade authority.
echo "admin set-admin -u l -v --program passport --fee-payer another_payer.json $(solana address)"
$CLI_BIN admin set-admin \
    -u l \
    -v \
    --program passport \
    --fee-payer another_payer.json \
    $(solana address)
echo

### Set admin to bogus address.
echo "admin set-admin -u l -v --program revenue-distribution devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj"
$CLI_BIN admin set-admin \
    -u l \
    -v \
    --program revenue-distribution \
    devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj
echo

### Set admin to upgrade authority.
echo "admin set-admin -u l -v --program revenue-distribution --fee-payer another_payer.json $(solana address)"
$CLI_BIN admin set-admin \
    -u l \
    -v \
    --program revenue-distribution \
    --fee-payer another_payer.json \
    $(solana address)
echo

### ATA commands.

$CLI_BIN ata -h
echo

### Contributor commands.

$CLI_BIN contributor -h
echo

### Prepaid commands.

$CLI_BIN prepaid -h
echo

### Validator commands.

$CLI_BIN validator -h
echo
