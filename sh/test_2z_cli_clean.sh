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

DUMMY_KEY=devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj

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
    $DUMMY_KEY
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
    $DUMMY_KEY
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

### Configure program.

$CLI_BIN admin configure -h
echo

### Configure passport.

$CLI_BIN admin configure passport -h
echo

echo "admin configure passport -u l -v --pause"
$CLI_BIN admin configure passport -u l -v --pause
echo

echo "admin configure passport -u l -v --unpause"
$CLI_BIN admin configure passport -u l -v --unpause
echo

### Configure revenue distribution.

$CLI_BIN admin configure revenue-distribution -h
echo

echo "admin configure revenue-distribution -u l -v --pause"
$CLI_BIN admin configure revenue-distribution \
    -u l \
    -v \
    --pause \
    --payments-accountant $DUMMY_KEY \
    --rewards-accountant $DUMMY_KEY \
    --sol-2z-swap-program $DUMMY_KEY
echo

echo "admin configure revenue-distribution -u l -v --unpause"
$CLI_BIN admin configure revenue-distribution -u l -v --unpause
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

### Clean up.

echo "rm another_payer.json"
rm another_payer.json
