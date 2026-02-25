use {
    crate::PriceFeed,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_instruction,
    },
};

/// Instructions supported by the Oracle Adapter program.
#[derive(Clone, Debug, PartialEq, serde_derive::Deserialize, serde_derive::Serialize)]
pub enum OracleInstruction {
    /// Initialize a new price feed account.
    ///
    /// Accounts expected:
    ///   0. `[writable, signer]` Price feed account to initialize.
    ///   1. `[]` Authority that is allowed to submit price updates.
    InitializePriceFeed {
        /// The exponent applied to all price values stored in this feed.
        exponent: i32,
    },

    /// Submit a price update to an existing price feed.
    ///
    /// Accounts expected:
    ///   0. `[writable]` Price feed account to update.
    ///   1. `[signer]`   Authority that was set during initialization.
    UpdatePrice {
        /// New price value (before applying the exponent).
        price: i64,
        /// Confidence interval for the new price.
        confidence: u64,
        /// Unix timestamp of this price observation.
        timestamp: i64,
    },

    /// Transfer the feed authority to a new public key.
    ///
    /// Accounts expected:
    ///   0. `[writable]` Price feed account.
    ///   1. `[signer]`   Current authority.
    ///   2. `[]`         New authority.
    SetAuthority,

    /// Mark a price feed as invalid without changing the price data.
    /// Useful for pausing a feed during maintenance or in emergency situations.
    ///
    /// Accounts expected:
    ///   0. `[writable]` Price feed account.
    ///   1. `[signer]`   Authority that was set during initialization.
    InvalidateFeed,

    /// Close a price feed account and transfer its lamports to a recipient.
    ///
    /// Accounts expected:
    ///   0. `[writable]` Price feed account to close.
    ///   1. `[signer]`   Authority that was set during initialization.
    ///   2. `[writable]` Recipient account that will receive the reclaimed lamports.
    CloseFeed,
}

/// Build the instructions required to create and initialize a new price feed
/// account owned by the Oracle Adapter program.
pub fn create_price_feed(
    from_pubkey: &Pubkey,
    price_feed_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    lamports: u64,
    exponent: i32,
) -> Vec<Instruction> {
    let space = PriceFeed::max_space();
    vec![
        system_instruction::create_account(
            from_pubkey,
            price_feed_pubkey,
            lamports,
            space,
            &crate::id(),
        ),
        initialize_price_feed(price_feed_pubkey, authority_pubkey, exponent),
    ]
}

/// Build the `InitializePriceFeed` instruction.
pub fn initialize_price_feed(
    price_feed_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    exponent: i32,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*price_feed_pubkey, true),
        AccountMeta::new_readonly(*authority_pubkey, false),
    ];
    Instruction::new_with_bincode(
        crate::id(),
        &OracleInstruction::InitializePriceFeed { exponent },
        account_metas,
    )
}

/// Build the `UpdatePrice` instruction.
pub fn update_price(
    price_feed_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    price: i64,
    confidence: u64,
    timestamp: i64,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*price_feed_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, true),
    ];
    Instruction::new_with_bincode(
        crate::id(),
        &OracleInstruction::UpdatePrice {
            price,
            confidence,
            timestamp,
        },
        account_metas,
    )
}

/// Build the `SetAuthority` instruction.
pub fn set_authority(
    price_feed_pubkey: &Pubkey,
    current_authority_pubkey: &Pubkey,
    new_authority_pubkey: &Pubkey,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*price_feed_pubkey, false),
        AccountMeta::new_readonly(*current_authority_pubkey, true),
        AccountMeta::new_readonly(*new_authority_pubkey, false),
    ];
    Instruction::new_with_bincode(
        crate::id(),
        &OracleInstruction::SetAuthority,
        account_metas,
    )
}

/// Build the `InvalidateFeed` instruction.
pub fn invalidate_feed(
    price_feed_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*price_feed_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, true),
    ];
    Instruction::new_with_bincode(
        crate::id(),
        &OracleInstruction::InvalidateFeed,
        account_metas,
    )
}

/// Build the `CloseFeed` instruction.
pub fn close_feed(
    price_feed_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    recipient_pubkey: &Pubkey,
) -> Instruction {
    let account_metas = vec![
        AccountMeta::new(*price_feed_pubkey, false),
        AccountMeta::new_readonly(*authority_pubkey, true),
        AccountMeta::new(*recipient_pubkey, false),
    ];
    Instruction::new_with_bincode(
        crate::id(),
        &OracleInstruction::CloseFeed,
        account_metas,
    )
}
