#!/bin/bash

set -eu

CLI_BIN=target/release/2z

$CLI_BIN -h
echo

### Establish another payer.

echo "solana-keygen new --silent --no-bip39-passphrase -o another_payer.json"
solana-keygen new --silent --no-bip39-passphrase -o another_payer.json
solana airdrop -u l 69 -k another_payer.json

### Admin commands.

$CLI_BIN admin -h
echo

echo "admin initialize -u l -v"
$CLI_BIN admin initialize -u l -v
echo

echo "admin set-admin -u l -v devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj"
$CLI_BIN admin set-admin -u l -v devgM7SXHvoHH6jPXRsjn97gygPUo58XEnc9bqY1jpj
echo

echo "admin set-admin -u l -v --fee-payer another_payer.json $(solana address)"
$CLI_BIN admin set-admin -u l -v --fee-payer another_payer.json $(solana address)
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
