# New Genesis File

This directory contains a newly generated Solana genesis configuration for a development cluster.

## Files Created

- **genesis.bin** - The genesis block binary file (23 KB)
- **genesis.tar.bz2** - Compressed archive of the genesis data (26 KB)
- **faucet.json** - Keypair for the faucet account that holds initial funds
- **identity.json** - Bootstrap validator identity keypair
- **vote-account.json** - Bootstrap validator vote account keypair
- **stake-account.json** - Bootstrap validator stake account keypair
- **rocksdb/** - RocksDB ledger database directory

## Genesis Configuration

- **Genesis Hash**: CVMZyYyUiTBnd87hwmvs1NEPjpghNhwjUbH5wgsPDL9d
- **Cluster Type**: Development
- **Creation Time**: 2026-01-28T22:20:23+00:00
- **Shred Version**: 52103
- **Ticks per Slot**: 64
- **Hashes per Tick**: 481
- **Target Tick Duration**: 6.25ms
- **Slots per Epoch**: 8192
- **Warmup Epochs**: Disabled
- **Initial Capitalization**: 500000500.7063431 SOL in 192 accounts
- **Faucet Lamports**: 500000000000000000 (500M SOL)

## How This Was Created

The genesis file was created using the `solana-genesis` binary with the following steps:

1. Built the required binaries:
   ```bash
   cargo build --bin solana-genesis --bin solana-keygen
   ```

2. Generated the required keypairs:
   ```bash
   target/debug/solana-keygen new --no-passphrase -fso genesis-output/faucet.json
   target/debug/solana-keygen new --no-passphrase -so genesis-output/identity.json
   target/debug/solana-keygen new --no-passphrase -so genesis-output/vote-account.json
   target/debug/solana-keygen new --no-passphrase -so genesis-output/stake-account.json
   ```

3. Created the genesis file:
   ```bash
   target/debug/solana-genesis \
     --ledger genesis-output \
     --bootstrap-validator genesis-output/identity.json \
                            genesis-output/vote-account.json \
                            genesis-output/stake-account.json \
     --faucet-pubkey genesis-output/faucet.json \
     --faucet-lamports 500000000000000000 \
     --hashes-per-tick auto \
     --cluster-type development
   ```

## Usage

To use this genesis file with a Solana validator:

```bash
solana-validator --ledger /path/to/genesis-output
```

Or copy the genesis files to your validator's ledger directory:

```bash
cp genesis-output/genesis.bin /path/to/validator/ledger/
cp genesis-output/genesis.tar.bz2 /path/to/validator/ledger/
```

## Security Note

⚠️ **WARNING**: The keypair files in this directory contain private keys. This genesis configuration is for development purposes only. Never use these keypairs or this genesis configuration in a production environment or with real funds.
