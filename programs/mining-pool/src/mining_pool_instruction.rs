//! Mining Pool instruction types

use {
    serde::{Deserialize, Serialize},
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
    },
};

/// Instructions supported by the Mining Pool program
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum MiningPoolInstruction {
    /// Initialize a new mining pool
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable, signer]` Pool account - to be created
    /// 1. `[signer]` Operator account - pool manager
    /// 2. `[]` Vote account - validator to link to
    /// 3. `[]` System program
    /// 4. `[]` Clock sysvar
    InitializePool {
        /// Pool commission percentage (0-100)
        pool_commission: u8,
    },

    /// Register a worker to a mining pool
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable, signer]` Worker account - to be created
    /// 1. `[signer]` Staker account
    /// 2. `[]` Stake account
    /// 3. `[writable]` Pool account
    /// 4. `[]` System program
    /// 5. `[]` Clock sysvar
    RegisterWorker {
        /// Amount of stake
        stake_amount: u64,
    },

    /// Register an operator
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable, signer]` Operator account - to be created
    /// 1. `[signer]` Operator authority
    /// 2. `[]` System program
    /// 3. `[]` Clock sysvar
    RegisterOperator,

    /// Link a pool to an operator
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable]` Operator account
    /// 1. `[signer]` Operator authority
    /// 2. `[]` Pool account
    LinkPoolToOperator,

    /// Update pool commission
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable]` Pool account
    /// 1. `[signer]` Operator authority
    UpdatePoolCommission {
        /// New commission percentage (0-100)
        new_commission: u8,
    },

    /// Deactivate a worker
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable]` Worker account
    /// 1. `[signer]` Staker or operator authority
    /// 2. `[writable]` Pool account
    DeactivateWorker,

    /// Deactivate a pool
    ///
    /// Accounts expected by this instruction:
    /// 0. `[writable]` Pool account
    /// 1. `[signer]` Operator authority
    DeactivatePool,
}

/// Create an `InitializePool` instruction
pub fn initialize_pool(
    program_id: &Pubkey,
    pool_account: &Pubkey,
    operator: &Pubkey,
    vote_account: &Pubkey,
    pool_commission: u8,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*pool_account, true),
        AccountMeta::new_readonly(*operator, true),
        AccountMeta::new_readonly(*vote_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::InitializePool { pool_commission },
        accounts,
    )
}

/// Create a `RegisterWorker` instruction
pub fn register_worker(
    program_id: &Pubkey,
    worker_account: &Pubkey,
    staker: &Pubkey,
    stake_account: &Pubkey,
    pool_account: &Pubkey,
    stake_amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*worker_account, true),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_account, false),
        AccountMeta::new(*pool_account, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::RegisterWorker { stake_amount },
        accounts,
    )
}

/// Create a `RegisterOperator` instruction
pub fn register_operator(
    program_id: &Pubkey,
    operator_account: &Pubkey,
    operator_authority: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*operator_account, true),
        AccountMeta::new_readonly(*operator_authority, true),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::RegisterOperator,
        accounts,
    )
}

/// Create a `LinkPoolToOperator` instruction
pub fn link_pool_to_operator(
    program_id: &Pubkey,
    operator_account: &Pubkey,
    operator_authority: &Pubkey,
    pool_account: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*operator_account, false),
        AccountMeta::new_readonly(*operator_authority, true),
        AccountMeta::new_readonly(*pool_account, false),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::LinkPoolToOperator,
        accounts,
    )
}

/// Create an `UpdatePoolCommission` instruction
pub fn update_pool_commission(
    program_id: &Pubkey,
    pool_account: &Pubkey,
    operator_authority: &Pubkey,
    new_commission: u8,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*pool_account, false),
        AccountMeta::new_readonly(*operator_authority, true),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::UpdatePoolCommission { new_commission },
        accounts,
    )
}

/// Create a `DeactivateWorker` instruction
pub fn deactivate_worker(
    program_id: &Pubkey,
    worker_account: &Pubkey,
    authority: &Pubkey,
    pool_account: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*worker_account, false),
        AccountMeta::new_readonly(*authority, true),
        AccountMeta::new(*pool_account, false),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::DeactivateWorker,
        accounts,
    )
}

/// Create a `DeactivatePool` instruction
pub fn deactivate_pool(
    program_id: &Pubkey,
    pool_account: &Pubkey,
    operator_authority: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*pool_account, false),
        AccountMeta::new_readonly(*operator_authority, true),
    ];
    
    Instruction::new_with_bincode(
        *program_id,
        &MiningPoolInstruction::DeactivatePool,
        accounts,
    )
}
