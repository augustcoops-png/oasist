//! Integration tests for the Mining Pool program

use {
    solana_mining_pool_program::{
        mining_pool_instruction::{
            deactivate_pool, deactivate_worker, initialize_pool, link_pool_to_operator,
            register_operator, register_worker, update_pool_commission,
        },
        mining_pool_state::{MiningPoolState, PoolOperator, PoolWorker},
    },
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer},
    },
};

#[test]
fn test_initialize_pool_instruction() {
    let program_id = solana_mining_pool_program::id();
    let pool_keypair = Keypair::new();
    let operator = Keypair::new();
    let vote_account = Pubkey::new_unique();
    let commission = 10u8;

    let instruction = initialize_pool(
        &program_id,
        &pool_keypair.pubkey(),
        &operator.pubkey(),
        &vote_account,
        commission,
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 5);
}

#[test]
fn test_register_worker_instruction() {
    let program_id = solana_mining_pool_program::id();
    let worker_keypair = Keypair::new();
    let staker = Keypair::new();
    let stake_account = Pubkey::new_unique();
    let pool_account = Pubkey::new_unique();
    let stake_amount = 1000u64;

    let instruction = register_worker(
        &program_id,
        &worker_keypair.pubkey(),
        &staker.pubkey(),
        &stake_account,
        &pool_account,
        stake_amount,
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 6);
}

#[test]
fn test_register_operator_instruction() {
    let program_id = solana_mining_pool_program::id();
    let operator_keypair = Keypair::new();
    let operator_authority = Keypair::new();

    let instruction = register_operator(
        &program_id,
        &operator_keypair.pubkey(),
        &operator_authority.pubkey(),
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 4);
}

#[test]
fn test_link_pool_to_operator_instruction() {
    let program_id = solana_mining_pool_program::id();
    let operator_account = Pubkey::new_unique();
    let operator_authority = Keypair::new();
    let pool_account = Pubkey::new_unique();

    let instruction = link_pool_to_operator(
        &program_id,
        &operator_account,
        &operator_authority.pubkey(),
        &pool_account,
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 3);
}

#[test]
fn test_update_pool_commission_instruction() {
    let program_id = solana_mining_pool_program::id();
    let pool_account = Pubkey::new_unique();
    let operator_authority = Keypair::new();
    let new_commission = 15u8;

    let instruction = update_pool_commission(
        &program_id,
        &pool_account,
        &operator_authority.pubkey(),
        new_commission,
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 2);
}

#[test]
fn test_deactivate_worker_instruction() {
    let program_id = solana_mining_pool_program::id();
    let worker_account = Pubkey::new_unique();
    let authority = Keypair::new();
    let pool_account = Pubkey::new_unique();

    let instruction = deactivate_worker(
        &program_id,
        &worker_account,
        &authority.pubkey(),
        &pool_account,
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 3);
}

#[test]
fn test_deactivate_pool_instruction() {
    let program_id = solana_mining_pool_program::id();
    let pool_account = Pubkey::new_unique();
    let operator_authority = Keypair::new();

    let instruction = deactivate_pool(
        &program_id,
        &pool_account,
        &operator_authority.pubkey(),
    );

    assert_eq!(instruction.program_id, program_id);
    assert_eq!(instruction.accounts.len(), 2);
}

#[test]
fn test_mining_pool_state_serialization() {
    let operator = Pubkey::new_unique();
    let vote_account = Pubkey::new_unique();
    let pool = MiningPoolState::new(operator, vote_account, 10, 0);

    // Test serialization
    let serialized = bincode::serialize(&pool).unwrap();
    assert!(!serialized.is_empty());

    // Test deserialization
    let deserialized: MiningPoolState = bincode::deserialize(&serialized).unwrap();
    assert_eq!(pool, deserialized);
}

#[test]
fn test_pool_worker_serialization() {
    let staker = Pubkey::new_unique();
    let stake_account = Pubkey::new_unique();
    let pool_account = Pubkey::new_unique();
    let worker = PoolWorker::new(staker, stake_account, pool_account, 1000, 0);

    // Test serialization
    let serialized = bincode::serialize(&worker).unwrap();
    assert!(!serialized.is_empty());

    // Test deserialization
    let deserialized: PoolWorker = bincode::deserialize(&serialized).unwrap();
    assert_eq!(worker, deserialized);
}

#[test]
fn test_pool_operator_serialization() {
    let operator = Pubkey::new_unique();
    let mut op = PoolOperator::new(operator, 0);
    
    let pool1 = Pubkey::new_unique();
    op.add_pool(pool1);

    // Test serialization
    let serialized = bincode::serialize(&op).unwrap();
    assert!(!serialized.is_empty());

    // Test deserialization
    let deserialized: PoolOperator = bincode::deserialize(&serialized).unwrap();
    assert_eq!(op, deserialized);
}
