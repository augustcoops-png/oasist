//! Mining Pool state definitions

use {
    serde::{Deserialize, Serialize},
    solana_sdk::{
        clock::Epoch,
        pubkey::Pubkey,
    },
};

/// Maximum number of workers that can be linked to a mining pool
pub const MAX_POOL_WORKERS: usize = 1000;

/// Mining Pool State
/// Links a pool operator to a vote account and manages pool workers
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MiningPoolState {
    /// The operator who manages the mining pool
    pub operator: Pubkey,
    
    /// The vote account this pool is linked to
    pub vote_account: Pubkey,
    
    /// Total delegated stake in the pool
    pub total_delegated_stake: u64,
    
    /// Pool commission percentage (0-100)
    pub pool_commission: u8,
    
    /// Number of active workers
    pub worker_count: u32,
    
    /// Epoch when the pool was created
    pub creation_epoch: Epoch,
    
    /// Whether the pool is currently active
    pub is_active: bool,
}

impl MiningPoolState {
    /// Create a new mining pool state
    pub fn new(
        operator: Pubkey,
        vote_account: Pubkey,
        pool_commission: u8,
        creation_epoch: Epoch,
    ) -> Self {
        Self {
            operator,
            vote_account,
            total_delegated_stake: 0,
            pool_commission,
            worker_count: 0,
            creation_epoch,
            is_active: true,
        }
    }
    
    /// Check if commission is valid (0-100)
    pub fn is_valid_commission(commission: u8) -> bool {
        commission <= 100
    }
}

/// Pool Worker
/// Links an individual staker to a mining pool
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct PoolWorker {
    /// The staker's public key
    pub staker: Pubkey,
    
    /// The stake account linked to this worker
    pub stake_account: Pubkey,
    
    /// The mining pool this worker is linked to
    pub pool_account: Pubkey,
    
    /// Worker's delegated stake amount
    pub stake_amount: u64,
    
    /// Epoch when worker joined the pool
    pub join_epoch: Epoch,
    
    /// Whether this worker is currently active
    pub is_active: bool,
}

impl PoolWorker {
    /// Create a new pool worker
    pub fn new(
        staker: Pubkey,
        stake_account: Pubkey,
        pool_account: Pubkey,
        stake_amount: u64,
        join_epoch: Epoch,
    ) -> Self {
        Self {
            staker,
            stake_account,
            pool_account,
            stake_amount,
            join_epoch,
            is_active: true,
        }
    }
}

/// Pool Operator Metadata
/// Additional metadata about the pool operator
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct PoolOperator {
    /// The operator's public key
    pub operator: Pubkey,
    
    /// List of mining pools managed by this operator
    pub managed_pools: Vec<Pubkey>,
    
    /// Total stake across all managed pools
    pub total_managed_stake: u64,
    
    /// Operator registration epoch
    pub registration_epoch: Epoch,
}

impl PoolOperator {
    /// Create a new pool operator
    pub fn new(operator: Pubkey, registration_epoch: Epoch) -> Self {
        Self {
            operator,
            managed_pools: Vec::new(),
            total_managed_stake: 0,
            registration_epoch,
        }
    }
    
    /// Add a pool to the operator's managed pools
    pub fn add_pool(&mut self, pool: Pubkey) {
        if !self.managed_pools.contains(&pool) {
            self.managed_pools.push(pool);
        }
    }
    
    /// Remove a pool from the operator's managed pools
    pub fn remove_pool(&mut self, pool: &Pubkey) {
        self.managed_pools.retain(|p| p != pool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mining_pool_state_new() {
        let operator = Pubkey::new_unique();
        let vote_account = Pubkey::new_unique();
        let pool = MiningPoolState::new(operator, vote_account, 10, 0);
        
        assert_eq!(pool.operator, operator);
        assert_eq!(pool.vote_account, vote_account);
        assert_eq!(pool.pool_commission, 10);
        assert_eq!(pool.worker_count, 0);
        assert!(pool.is_active);
    }
    
    #[test]
    fn test_valid_commission() {
        assert!(MiningPoolState::is_valid_commission(0));
        assert!(MiningPoolState::is_valid_commission(50));
        assert!(MiningPoolState::is_valid_commission(100));
        assert!(!MiningPoolState::is_valid_commission(101));
    }
    
    #[test]
    fn test_pool_worker_new() {
        let staker = Pubkey::new_unique();
        let stake_account = Pubkey::new_unique();
        let pool_account = Pubkey::new_unique();
        let worker = PoolWorker::new(staker, stake_account, pool_account, 1000, 0);
        
        assert_eq!(worker.staker, staker);
        assert_eq!(worker.stake_account, stake_account);
        assert_eq!(worker.pool_account, pool_account);
        assert_eq!(worker.stake_amount, 1000);
        assert!(worker.is_active);
    }
    
    #[test]
    fn test_pool_operator_add_remove_pool() {
        let operator = Pubkey::new_unique();
        let mut op = PoolOperator::new(operator, 0);
        
        let pool1 = Pubkey::new_unique();
        let pool2 = Pubkey::new_unique();
        
        op.add_pool(pool1);
        assert_eq!(op.managed_pools.len(), 1);
        
        op.add_pool(pool2);
        assert_eq!(op.managed_pools.len(), 2);
        
        // Adding same pool shouldn't duplicate
        op.add_pool(pool1);
        assert_eq!(op.managed_pools.len(), 2);
        
        op.remove_pool(&pool1);
        assert_eq!(op.managed_pools.len(), 1);
        assert_eq!(op.managed_pools[0], pool2);
    }
}
