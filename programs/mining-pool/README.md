# Mining Pool Program

The Mining Pool Program provides functionality for linking mining pools, pool workers, and pool operators in the Solana blockchain ecosystem.

> **Note**: This program uses "mining pool" terminology for consistency with the requirements, but it actually implements stake pooling for validators in Solana's Proof of Stake consensus system, not traditional Proof of Work mining.

## Overview

This program enables:
- **Mining Pool Creation**: Operators can create mining pools linked to vote accounts
- **Worker Registration**: Individual stakers can join pools as workers
- **Operator Management**: Track and manage multiple pools under a single operator
- **Commission Control**: Configure and update pool commission rates
- **Pool/Worker Lifecycle**: Activate and deactivate pools and workers

## Limits

- **MAX_POOL_WORKERS**: 1,000 workers per pool
- **MAX_MANAGED_POOLS**: 100 pools per operator

## Architecture

### Core Components

1. **MiningPoolState** - Represents a mining pool
   - Linked to a vote account (validator)
   - Managed by an operator
   - Tracks total delegated stake
   - Manages pool commission (0-100%)
   - Monitors active worker count

2. **PoolWorker** - Represents an individual staker in a pool
   - Links stake account to pool
   - Tracks stake amount
   - Records join epoch
   - Can be activated/deactivated

3. **PoolOperator** - Manages multiple mining pools
   - Can create and manage multiple pools
   - Tracks total managed stake across pools
   - Provides centralized operator control

## Instructions

### InitializePool
Creates a new mining pool linked to a vote account.

**Accounts:**
- Pool account (writable, signer) - New pool to create
- Operator account (signer) - Pool manager
- Vote account - Validator to link to
- System program
- Clock sysvar

**Parameters:**
- `pool_commission`: u8 (0-100) - Commission percentage

### RegisterWorker
Registers a worker (staker) to a mining pool.

**Accounts:**
- Worker account (writable, signer) - New worker account
- Staker account (signer) - Staker identity
- Stake account - Associated stake account
- Pool account (writable) - Target pool
- System program
- Clock sysvar

**Parameters:**
- `stake_amount`: u64 - Amount of stake

### RegisterOperator
Registers a new pool operator.

**Accounts:**
- Operator account (writable, signer) - New operator account
- Operator authority (signer) - Operator identity
- System program
- Clock sysvar

### LinkPoolToOperator
Links an existing pool to an operator's managed pools.

**Accounts:**
- Operator account (writable) - Operator to link to
- Operator authority (signer) - Must match operator
- Pool account - Pool to link

### UpdatePoolCommission
Updates the commission rate for a pool.

**Accounts:**
- Pool account (writable) - Pool to update
- Operator authority (signer) - Must be pool operator

**Parameters:**
- `new_commission`: u8 (0-100) - New commission percentage

### DeactivateWorker
Deactivates a worker from a pool.

**Accounts:**
- Worker account (writable) - Worker to deactivate
- Authority (signer) - Staker or pool operator
- Pool account (writable) - Associated pool

### DeactivatePool
Deactivates a mining pool.

**Accounts:**
- Pool account (writable) - Pool to deactivate
- Operator authority (signer) - Must be pool operator

## Usage Example

```rust
use solana_mining_pool_program::{
    mining_pool_instruction::{initialize_pool, register_worker},
    id,
};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};

// Create a new mining pool
let pool_keypair = Keypair::new();
let operator = Keypair::new();
let vote_account = /* existing vote account */;

let ix = initialize_pool(
    &id(),
    &pool_keypair.pubkey(),
    &operator.pubkey(),
    &vote_account,
    10, // 10% commission
);

// Register a worker
let worker_keypair = Keypair::new();
let staker = Keypair::new();
let stake_account = /* existing stake account */;

let ix2 = register_worker(
    &id(),
    &worker_keypair.pubkey(),
    &staker.pubkey(),
    &stake_account,
    &pool_keypair.pubkey(),
    1000000, // stake amount in lamports
);
```

## Security Considerations

- Only pool operators can update pool settings
- Workers can only be deactivated by the staker or pool operator
- Commission rates are validated to be within 0-100%
- All state changes are validated before execution
- Stake amounts must be non-zero
- Worker and pool limits are enforced
- Authorization checks use MissingRequiredSignature error for better debugging
- Pool account matching is verified for worker operations

## Program ID

```
MPoo111111111111111111111111111111111111111
```

## Building

```bash
cargo build-bpf
```

## Testing

```bash
cargo test
```

## License

Same as the Solana repository (Apache 2.0)
