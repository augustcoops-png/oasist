#![allow(clippy::arithmetic_side_effects)]
pub mod oracle_instruction;
pub mod oracle_processor;

use {
    bincode::{deserialize, serialize},
    serde_derive::{Deserialize, Serialize},
    solana_sdk::{
        account::{Account, AccountSharedData},
        pubkey::Pubkey,
    },
};

solana_sdk::declare_id!("PriceFeed1111111111111111111111111111111111");

/// Represents the state of a price feed account managed by the oracle adapter.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PriceFeed {
    /// The public key authorized to submit price updates for this feed.
    pub authority: Pubkey,
    /// Current price value. The actual price is `price * 10^exponent`.
    pub price: i64,
    /// The exponent applied to `price` to obtain the real-world value.
    pub exponent: i32,
    /// Confidence interval around the current price (same exponent as `price`).
    pub confidence: u64,
    /// Unix timestamp (seconds) of the most recent price update.
    pub timestamp: i64,
    /// Whether this price feed contains a valid, up-to-date price.
    pub is_valid: bool,
}

impl PriceFeed {
    pub fn max_space() -> u64 {
        bincode::serialized_size(&PriceFeed::default()).unwrap()
    }
}

/// Utility function to create a pre-populated `PriceFeed` account for use in
/// genesis or testing.
pub fn create_price_feed_account(authority: Pubkey, lamports: u64) -> AccountSharedData {
    let feed = PriceFeed {
        authority,
        ..PriceFeed::default()
    };
    AccountSharedData::from(Account {
        lamports,
        data: serialize(&feed).unwrap(),
        owner: id(),
        ..Account::default()
    })
}

/// Deserialize a `PriceFeed` from raw account data.
pub fn get_price_feed_data(bytes: &[u8]) -> Result<PriceFeed, bincode::Error> {
    deserialize(bytes)
}
