//! Mining Pool processor

use {
    crate::{
        mining_pool_instruction::MiningPoolInstruction,
        mining_pool_state::{MiningPoolState, PoolOperator, PoolWorker},
    },
    log::*,
    solana_program_runtime::{ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        instruction::InstructionError,
        program_utils::limited_deserialize,
        transaction_context::{
            IndexOfAccount,
        },
    },
};

/// Process an instruction
pub fn process_instruction(
    _first_instruction_account: IndexOfAccount,
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_data = instruction_context.get_instruction_data();

    trace!("Mining pool instruction: {:?}", instruction_data);

    let instruction: MiningPoolInstruction = limited_deserialize(instruction_data)?;

    match instruction {
        MiningPoolInstruction::InitializePool { pool_commission } => {
            instruction_context.check_number_of_instruction_accounts(5)?;
            process_initialize_pool(invoke_context, pool_commission)
        }
        MiningPoolInstruction::RegisterWorker { stake_amount } => {
            instruction_context.check_number_of_instruction_accounts(6)?;
            process_register_worker(invoke_context, stake_amount)
        }
        MiningPoolInstruction::RegisterOperator => {
            instruction_context.check_number_of_instruction_accounts(4)?;
            process_register_operator(invoke_context)
        }
        MiningPoolInstruction::LinkPoolToOperator => {
            instruction_context.check_number_of_instruction_accounts(3)?;
            process_link_pool_to_operator(invoke_context)
        }
        MiningPoolInstruction::UpdatePoolCommission { new_commission } => {
            instruction_context.check_number_of_instruction_accounts(2)?;
            process_update_pool_commission(invoke_context, new_commission)
        }
        MiningPoolInstruction::DeactivateWorker => {
            instruction_context.check_number_of_instruction_accounts(3)?;
            process_deactivate_worker(invoke_context)
        }
        MiningPoolInstruction::DeactivatePool => {
            instruction_context.check_number_of_instruction_accounts(2)?;
            process_deactivate_pool(invoke_context)
        }
    }
}

/// Process `InitializePool` instruction
fn process_initialize_pool(
    invoke_context: &mut InvokeContext,
    pool_commission: u8,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let operator_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    let vote_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 2)?;
    
    // Validate commission
    if !MiningPoolState::is_valid_commission(pool_commission) {
        return Err(InstructionError::InvalidArgument);
    }
    
    // Check if pool account is already initialized
    if pool_account.get_data().len() > 0 {
        return Err(InstructionError::AccountAlreadyInitialized);
    }
    
    // Get clock for epoch
    let clock = invoke_context.get_sysvar_cache().get_clock()?;
    
    // Create new pool state
    let pool_state = MiningPoolState::new(
        *operator_account.get_key(),
        *vote_account.get_key(),
        pool_commission,
        clock.epoch,
    );
    
    // Serialize and store
    let serialized = bincode::serialize(&pool_state).map_err(|_| InstructionError::InvalidAccountData)?;
    pool_account.set_data_from_slice(&serialized)?;
    
    ic_msg!(
        invoke_context,
        "Initialized mining pool with operator {} and vote account {}",
        operator_account.get_key(),
        vote_account.get_key()
    );
    
    Ok(())
}

/// Process `RegisterWorker` instruction
fn process_register_worker(
    invoke_context: &mut InvokeContext,
    stake_amount: u64,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut worker_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let staker_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    let stake_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 2)?;
    let mut pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 3)?;
    
    // Check if worker account is already initialized
    if worker_account.get_data().len() > 0 {
        return Err(InstructionError::AccountAlreadyInitialized);
    }
    
    // Deserialize pool state
    let mut pool_state: MiningPoolState = bincode::deserialize(pool_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Check pool is active
    if !pool_state.is_active {
        return Err(InstructionError::InvalidAccountData);
    }
    
    // Get clock for epoch
    let clock = invoke_context.get_sysvar_cache().get_clock()?;
    
    // Create new worker
    let worker = PoolWorker::new(
        *staker_account.get_key(),
        *stake_account.get_key(),
        *pool_account.get_key(),
        stake_amount,
        clock.epoch,
    );
    
    // Update pool state
    pool_state.worker_count = pool_state.worker_count.saturating_add(1);
    pool_state.total_delegated_stake = pool_state.total_delegated_stake.saturating_add(stake_amount);
    
    // Serialize and store worker
    let worker_data = bincode::serialize(&worker).map_err(|_| InstructionError::InvalidAccountData)?;
    worker_account.set_data_from_slice(&worker_data)?;
    
    // Serialize and store updated pool state
    let pool_data = bincode::serialize(&pool_state).map_err(|_| InstructionError::InvalidAccountData)?;
    pool_account.set_data_from_slice(&pool_data)?;
    
    ic_msg!(
        invoke_context,
        "Registered worker {} to pool {}",
        staker_account.get_key(),
        pool_account.get_key()
    );
    
    Ok(())
}

/// Process `RegisterOperator` instruction
fn process_register_operator(
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut operator_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let operator_authority = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    
    // Check if operator account is already initialized
    if operator_account.get_data().len() > 0 {
        return Err(InstructionError::AccountAlreadyInitialized);
    }
    
    // Get clock for epoch
    let clock = invoke_context.get_sysvar_cache().get_clock()?;
    
    // Create new operator
    let operator = PoolOperator::new(
        *operator_authority.get_key(),
        clock.epoch,
    );
    
    // Serialize and store
    let serialized = bincode::serialize(&operator).map_err(|_| InstructionError::InvalidAccountData)?;
    operator_account.set_data_from_slice(&serialized)?;
    
    ic_msg!(
        invoke_context,
        "Registered operator {}",
        operator_authority.get_key()
    );
    
    Ok(())
}

/// Process `LinkPoolToOperator` instruction
fn process_link_pool_to_operator(
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut operator_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let operator_authority = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    let pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 2)?;
    
    // Deserialize operator
    let mut operator: PoolOperator = bincode::deserialize(operator_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Verify authority
    if operator.operator != *operator_authority.get_key() {
        return Err(InstructionError::InvalidAccountData);
    }
    
    // Deserialize pool to verify it exists and get its stake
    let pool_state: MiningPoolState = bincode::deserialize(pool_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Add pool to operator
    operator.add_pool(*pool_account.get_key());
    operator.total_managed_stake = operator.total_managed_stake
        .saturating_add(pool_state.total_delegated_stake);
    
    // Serialize and store updated operator
    let serialized = bincode::serialize(&operator).map_err(|_| InstructionError::InvalidAccountData)?;
    operator_account.set_data_from_slice(&serialized)?;
    
    ic_msg!(
        invoke_context,
        "Linked pool {} to operator {}",
        pool_account.get_key(),
        operator_authority.get_key()
    );
    
    Ok(())
}

/// Process `UpdatePoolCommission` instruction
fn process_update_pool_commission(
    invoke_context: &mut InvokeContext,
    new_commission: u8,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let operator_authority = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    
    // Validate commission
    if !MiningPoolState::is_valid_commission(new_commission) {
        return Err(InstructionError::InvalidArgument);
    }
    
    // Deserialize pool state
    let mut pool_state: MiningPoolState = bincode::deserialize(pool_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Verify authority
    if pool_state.operator != *operator_authority.get_key() {
        return Err(InstructionError::InvalidAccountData);
    }
    
    // Update commission
    pool_state.pool_commission = new_commission;
    
    // Serialize and store
    let serialized = bincode::serialize(&pool_state).map_err(|_| InstructionError::InvalidAccountData)?;
    pool_account.set_data_from_slice(&serialized)?;
    
    ic_msg!(
        invoke_context,
        "Updated pool commission to {}",
        new_commission
    );
    
    Ok(())
}

/// Process `DeactivateWorker` instruction
fn process_deactivate_worker(
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut worker_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let authority = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    let mut pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 2)?;
    
    // Deserialize worker
    let mut worker: PoolWorker = bincode::deserialize(worker_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Deserialize pool
    let mut pool_state: MiningPoolState = bincode::deserialize(pool_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Verify authority (must be staker or pool operator)
    if worker.staker != *authority.get_key() && pool_state.operator != *authority.get_key() {
        return Err(InstructionError::InvalidAccountData);
    }
    
    // Deactivate worker
    worker.is_active = false;
    
    // Update pool state
    pool_state.worker_count = pool_state.worker_count.saturating_sub(1);
    pool_state.total_delegated_stake = pool_state.total_delegated_stake.saturating_sub(worker.stake_amount);
    
    // Serialize and store
    let worker_data = bincode::serialize(&worker).map_err(|_| InstructionError::InvalidAccountData)?;
    worker_account.set_data_from_slice(&worker_data)?;
    
    let pool_data = bincode::serialize(&pool_state).map_err(|_| InstructionError::InvalidAccountData)?;
    pool_account.set_data_from_slice(&pool_data)?;
    
    ic_msg!(
        invoke_context,
        "Deactivated worker {}",
        worker_account.get_key()
    );
    
    Ok(())
}

/// Process `DeactivatePool` instruction
fn process_deactivate_pool(
    invoke_context: &mut InvokeContext,
) -> Result<(), InstructionError> {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    
    let mut pool_account = instruction_context
        .try_borrow_instruction_account(transaction_context, 0)?;
    let operator_authority = instruction_context
        .try_borrow_instruction_account(transaction_context, 1)?;
    
    // Deserialize pool state
    let mut pool_state: MiningPoolState = bincode::deserialize(pool_account.get_data())
        .map_err(|_| InstructionError::InvalidAccountData)?;
    
    // Verify authority
    if pool_state.operator != *operator_authority.get_key() {
        return Err(InstructionError::InvalidAccountData);
    }
    
    // Deactivate pool
    pool_state.is_active = false;
    
    // Serialize and store
    let serialized = bincode::serialize(&pool_state).map_err(|_| InstructionError::InvalidAccountData)?;
    pool_account.set_data_from_slice(&serialized)?;
    
    ic_msg!(
        invoke_context,
        "Deactivated pool {}",
        pool_account.get_key()
    );
    
    Ok(())
}
