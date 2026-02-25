//! Oracle Adapter program processor

use {
    crate::{oracle_error::OracleError, oracle_instruction::OracleInstruction, PriceFeed},
    bincode::{deserialize, serialize},
    solana_program_runtime::{declare_process_instruction, ic_msg},
    solana_sdk::{instruction::InstructionError, program_utils::limited_deserialize},
};

pub const DEFAULT_COMPUTE_UNITS: u64 = 450;

declare_process_instruction!(Entrypoint, DEFAULT_COMPUTE_UNITS, |invoke_context| {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_data = instruction_context.get_instruction_data();

    let instruction: OracleInstruction = limited_deserialize(instruction_data)?;

    match instruction {
        OracleInstruction::InitializePriceFeed { exponent } => {
            // Account 0: price feed account (writable, signer)
            // Account 1: authority (read-only)
            let price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;

            if price_feed_account.get_owner() != &crate::id() {
                ic_msg!(
                    invoke_context,
                    "InitializePriceFeed: price feed account must be owned by the oracle adapter program"
                );
                return Err(OracleError::NotFeedOwner.into());
            }
            if !price_feed_account.is_signer() {
                ic_msg!(
                    invoke_context,
                    "InitializePriceFeed: price feed account must be a signer"
                );
                return Err(InstructionError::MissingRequiredSignature);
            }
            drop(price_feed_account);

            let authority_key = *transaction_context.get_key_of_account_at_index(
                instruction_context.get_index_of_instruction_account_in_transaction(1)?,
            )?;

            let feed = PriceFeed {
                authority: authority_key,
                exponent,
                is_valid: false,
                ..PriceFeed::default()
            };
            let serialized = serialize(&feed).map_err(|err| {
                ic_msg!(
                    invoke_context,
                    "InitializePriceFeed: failed to serialize price feed: {}",
                    err
                );
                InstructionError::from(OracleError::InvalidFeedData)
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(OracleError::AccountDataTooSmall.into());
            }
            price_feed_account.get_data_mut()?[..serialized.len()].copy_from_slice(&serialized);
            Ok(())
        }

        OracleInstruction::UpdatePrice {
            price,
            confidence,
            timestamp,
        } => {
            // Account 0: price feed account (writable)
            // Account 1: authority (signer)
            let authority_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
            if !authority_account.is_signer() {
                ic_msg!(
                    invoke_context,
                    "UpdatePrice: authority must be a signer"
                );
                return Err(InstructionError::MissingRequiredSignature);
            }
            let authority_key = *authority_account.get_key();
            drop(authority_account);

            let price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;

            if price_feed_account.get_owner() != &crate::id() {
                ic_msg!(
                    invoke_context,
                    "UpdatePrice: price feed account must be owned by the oracle adapter program"
                );
                return Err(OracleError::NotFeedOwner.into());
            }

            let mut feed: PriceFeed =
                deserialize(price_feed_account.get_data()).map_err(|err| {
                    ic_msg!(
                        invoke_context,
                        "UpdatePrice: failed to deserialize price feed: {}",
                        err
                    );
                    InstructionError::from(OracleError::InvalidFeedData)
                })?;
            drop(price_feed_account);

            if feed.authority != authority_key {
                ic_msg!(
                    invoke_context,
                    "UpdatePrice: signer is not the feed authority"
                );
                return Err(OracleError::NotFeedAuthority.into());
            }

            feed.price = price;
            feed.confidence = confidence;
            feed.timestamp = timestamp;
            feed.is_valid = true;

            let serialized = serialize(&feed).map_err(|err| {
                ic_msg!(
                    invoke_context,
                    "UpdatePrice: failed to serialize price feed: {}",
                    err
                );
                InstructionError::from(OracleError::InvalidFeedData)
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(OracleError::AccountDataTooSmall.into());
            }
            price_feed_account.get_data_mut()?[..serialized.len()].copy_from_slice(&serialized);
            Ok(())
        }

        OracleInstruction::SetAuthority => {
            // Account 0: price feed account (writable)
            // Account 1: current authority (signer)
            // Account 2: new authority (read-only)
            let authority_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
            if !authority_account.is_signer() {
                ic_msg!(
                    invoke_context,
                    "SetAuthority: current authority must be a signer"
                );
                return Err(InstructionError::MissingRequiredSignature);
            }
            let current_authority_key = *authority_account.get_key();
            drop(authority_account);

            let new_authority_key = *transaction_context.get_key_of_account_at_index(
                instruction_context.get_index_of_instruction_account_in_transaction(2)?,
            )?;

            let price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_owner() != &crate::id() {
                ic_msg!(
                    invoke_context,
                    "SetAuthority: price feed account must be owned by the oracle adapter program"
                );
                return Err(OracleError::NotFeedOwner.into());
            }
            let mut feed: PriceFeed =
                deserialize(price_feed_account.get_data()).map_err(|err| {
                    ic_msg!(
                        invoke_context,
                        "SetAuthority: failed to deserialize price feed: {}",
                        err
                    );
                    InstructionError::from(OracleError::InvalidFeedData)
                })?;
            drop(price_feed_account);

            if feed.authority != current_authority_key {
                ic_msg!(
                    invoke_context,
                    "SetAuthority: signer is not the feed authority"
                );
                return Err(OracleError::NotFeedAuthority.into());
            }

            feed.authority = new_authority_key;

            let serialized = serialize(&feed).map_err(|err| {
                ic_msg!(
                    invoke_context,
                    "SetAuthority: failed to serialize price feed: {}",
                    err
                );
                InstructionError::from(OracleError::InvalidFeedData)
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(OracleError::AccountDataTooSmall.into());
            }
            price_feed_account.get_data_mut()?[..serialized.len()].copy_from_slice(&serialized);
            Ok(())
        }

        OracleInstruction::InvalidateFeed => {
            // Account 0: price feed account (writable)
            // Account 1: authority (signer)
            let authority_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
            if !authority_account.is_signer() {
                ic_msg!(
                    invoke_context,
                    "InvalidateFeed: authority must be a signer"
                );
                return Err(InstructionError::MissingRequiredSignature);
            }
            let authority_key = *authority_account.get_key();
            drop(authority_account);

            let price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_owner() != &crate::id() {
                ic_msg!(
                    invoke_context,
                    "InvalidateFeed: price feed account must be owned by the oracle adapter program"
                );
                return Err(OracleError::NotFeedOwner.into());
            }
            let mut feed: PriceFeed =
                deserialize(price_feed_account.get_data()).map_err(|err| {
                    ic_msg!(
                        invoke_context,
                        "InvalidateFeed: failed to deserialize price feed: {}",
                        err
                    );
                    InstructionError::from(OracleError::InvalidFeedData)
                })?;
            drop(price_feed_account);

            if feed.authority != authority_key {
                ic_msg!(
                    invoke_context,
                    "InvalidateFeed: signer is not the feed authority"
                );
                return Err(OracleError::NotFeedAuthority.into());
            }

            feed.is_valid = false;

            let serialized = serialize(&feed).map_err(|err| {
                ic_msg!(
                    invoke_context,
                    "InvalidateFeed: failed to serialize price feed: {}",
                    err
                );
                InstructionError::from(OracleError::InvalidFeedData)
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(OracleError::AccountDataTooSmall.into());
            }
            price_feed_account.get_data_mut()?[..serialized.len()].copy_from_slice(&serialized);
            Ok(())
        }

        OracleInstruction::CloseFeed => {
            // Account 0: price feed account (writable)
            // Account 1: authority (signer)
            // Account 2: recipient (writable)
            let authority_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 1)?;
            if !authority_account.is_signer() {
                ic_msg!(
                    invoke_context,
                    "CloseFeed: authority must be a signer"
                );
                return Err(InstructionError::MissingRequiredSignature);
            }
            let authority_key = *authority_account.get_key();
            drop(authority_account);

            let price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_owner() != &crate::id() {
                ic_msg!(
                    invoke_context,
                    "CloseFeed: price feed account must be owned by the oracle adapter program"
                );
                return Err(OracleError::NotFeedOwner.into());
            }
            let feed: PriceFeed =
                deserialize(price_feed_account.get_data()).map_err(|err| {
                    ic_msg!(
                        invoke_context,
                        "CloseFeed: failed to deserialize price feed: {}",
                        err
                    );
                    InstructionError::from(OracleError::InvalidFeedData)
                })?;
            if feed.authority != authority_key {
                ic_msg!(
                    invoke_context,
                    "CloseFeed: signer is not the feed authority"
                );
                return Err(OracleError::NotFeedAuthority.into());
            }
            let lamports = price_feed_account.get_lamports();
            drop(price_feed_account);

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            price_feed_account.set_lamports(0)?;
            drop(price_feed_account);

            let mut recipient_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 2)?;
            recipient_account.checked_add_lamports(lamports)?;
            Ok(())
        }
    }
});

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            get_price_feed_data, id, PriceFeed,
            oracle_instruction::{initialize_price_feed, update_price},
        },
        solana_program_runtime::invoke_context::mock_process_instruction,
        solana_sdk::{
            account::{AccountSharedData, ReadableAccount},
            instruction::AccountMeta,
            pubkey::Pubkey,
        },
    };

    fn process_instruction(
        instruction_data: &[u8],
        transaction_accounts: Vec<(Pubkey, AccountSharedData)>,
        instruction_accounts: Vec<AccountMeta>,
        expected_result: Result<(), InstructionError>,
    ) -> Vec<AccountSharedData> {
        mock_process_instruction(
            &id(),
            Vec::new(),
            instruction_data,
            transaction_accounts,
            instruction_accounts,
            expected_result,
            Entrypoint::vm,
            |_invoke_context| {},
            |_invoke_context| {},
        )
    }

    fn make_price_feed_account() -> AccountSharedData {
        AccountSharedData::new(1, PriceFeed::max_space() as usize, &id())
    }

    #[test]
    fn test_initialize_price_feed() {
        let feed_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let feed_account = make_price_feed_account();
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let instruction =
            initialize_price_feed(&feed_pubkey, &authority_pubkey, -8);

        let accounts = process_instruction(
            &instruction.data,
            vec![
                (feed_pubkey, feed_account),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta {
                    pubkey: feed_pubkey,
                    is_signer: true,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: authority_pubkey,
                    is_signer: false,
                    is_writable: false,
                },
            ],
            Ok(()),
        );

        let feed = get_price_feed_data(accounts[0].data()).unwrap();
        assert_eq!(feed.authority, authority_pubkey);
        assert_eq!(feed.exponent, -8);
        assert!(!feed.is_valid);
    }

    #[test]
    fn test_update_price() {
        let feed_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();

        // Initialize first
        let feed_account = make_price_feed_account();
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let init_instruction =
            initialize_price_feed(&feed_pubkey, &authority_pubkey, -8);
        let accounts = process_instruction(
            &init_instruction.data,
            vec![
                (feed_pubkey, feed_account),
                (authority_pubkey, authority_account.clone()),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: true, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: false, is_writable: false },
            ],
            Ok(()),
        );

        // Now update the price
        let initialized_feed_account = accounts[0].clone();
        let update_instruction =
            update_price(&feed_pubkey, &authority_pubkey, 5_000_000_000, 100_000, 1_700_000_000);
        let accounts = process_instruction(
            &update_instruction.data,
            vec![
                (feed_pubkey, initialized_feed_account),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: true, is_writable: false },
            ],
            Ok(()),
        );

        let feed = get_price_feed_data(accounts[0].data()).unwrap();
        assert_eq!(feed.authority, authority_pubkey);
        assert_eq!(feed.price, 5_000_000_000);
        assert_eq!(feed.confidence, 100_000);
        assert_eq!(feed.timestamp, 1_700_000_000);
        assert!(feed.is_valid);
    }

    #[test]
    fn test_update_price_wrong_authority() {
        let feed_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let wrong_authority_pubkey = Pubkey::new_unique();

        // Initialize
        let feed_account = make_price_feed_account();
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let init_instruction =
            initialize_price_feed(&feed_pubkey, &authority_pubkey, -8);
        let accounts = process_instruction(
            &init_instruction.data,
            vec![
                (feed_pubkey, feed_account),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: true, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: false, is_writable: false },
            ],
            Ok(()),
        );

        // Attempt update with wrong authority
        let initialized_feed_account = accounts[0].clone();
        let wrong_authority_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let update_instruction =
            update_price(&feed_pubkey, &wrong_authority_pubkey, 1_000, 10, 999);
        process_instruction(
            &update_instruction.data,
            vec![
                (feed_pubkey, initialized_feed_account),
                (wrong_authority_pubkey, wrong_authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: wrong_authority_pubkey, is_signer: true, is_writable: false },
            ],
            Err(InstructionError::MissingRequiredSignature),
        );
    }

    #[test]
    fn test_initialize_bad_owner() {
        let feed_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        // Account owned by a different program
        let bad_account = AccountSharedData::new(1, PriceFeed::max_space() as usize, &Pubkey::new_unique());
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let instruction = initialize_price_feed(&feed_pubkey, &authority_pubkey, -8);
        process_instruction(
            &instruction.data,
            vec![
                (feed_pubkey, bad_account),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: true, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: false, is_writable: false },
            ],
            Err(InstructionError::InvalidAccountOwner),
        );
    }

    // ---- helpers shared by the new tests ----------------------------------------

    /// Initialize a feed and return the resulting feed account state.
    fn init_feed(
        feed_pubkey: Pubkey,
        authority_pubkey: Pubkey,
        exponent: i32,
    ) -> AccountSharedData {
        let feed_account = make_price_feed_account();
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let ix = initialize_price_feed(&feed_pubkey, &authority_pubkey, exponent);
        let accounts = process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, feed_account),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: true, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: false, is_writable: false },
            ],
            Ok(()),
        );
        accounts[0].clone()
    }

    /// Initialize and then price-update a feed; returns the feed account state.
    fn init_and_update_feed(
        feed_pubkey: Pubkey,
        authority_pubkey: Pubkey,
        price: i64,
        confidence: u64,
        timestamp: i64,
    ) -> AccountSharedData {
        let initialized = init_feed(feed_pubkey, authority_pubkey, -8);
        let authority_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let ix = update_price(&feed_pubkey, &authority_pubkey, price, confidence, timestamp);
        let accounts = process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, initialized),
                (authority_pubkey, authority_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: authority_pubkey, is_signer: true, is_writable: false },
            ],
            Ok(()),
        );
        accounts[0].clone()
    }

    // ---- SetAuthority -----------------------------------------------------------

    #[test]
    fn test_set_authority() {
        use crate::oracle_instruction::set_authority;

        let feed_pubkey = Pubkey::new_unique();
        let old_authority = Pubkey::new_unique();
        let new_authority = Pubkey::new_unique();

        let initialized = init_feed(feed_pubkey, old_authority, -8);
        let old_auth_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let new_auth_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let ix = set_authority(&feed_pubkey, &old_authority, &new_authority);
        let accounts = process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, initialized),
                (old_authority, old_auth_account),
                (new_authority, new_auth_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: old_authority, is_signer: true, is_writable: false },
                AccountMeta { pubkey: new_authority, is_signer: false, is_writable: false },
            ],
            Ok(()),
        );

        let feed = get_price_feed_data(accounts[0].data()).unwrap();
        assert_eq!(feed.authority, new_authority);
    }

    #[test]
    fn test_set_authority_wrong_signer() {
        use crate::oracle_instruction::set_authority;

        let feed_pubkey = Pubkey::new_unique();
        let real_authority = Pubkey::new_unique();
        let wrong_signer = Pubkey::new_unique();
        let new_authority = Pubkey::new_unique();

        let initialized = init_feed(feed_pubkey, real_authority, -8);
        let wrong_signer_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let new_auth_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let ix = set_authority(&feed_pubkey, &wrong_signer, &new_authority);
        process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, initialized),
                (wrong_signer, wrong_signer_account),
                (new_authority, new_auth_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: wrong_signer, is_signer: true, is_writable: false },
                AccountMeta { pubkey: new_authority, is_signer: false, is_writable: false },
            ],
            Err(InstructionError::MissingRequiredSignature),
        );
    }

    // ---- InvalidateFeed ---------------------------------------------------------

    #[test]
    fn test_invalidate_feed() {
        use crate::oracle_instruction::invalidate_feed;

        let feed_pubkey = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        // Start with a valid feed
        let updated = init_and_update_feed(feed_pubkey, authority, 1_000, 5, 1_000_000);
        assert!(get_price_feed_data(updated.data()).unwrap().is_valid);

        let auth_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let ix = invalidate_feed(&feed_pubkey, &authority);
        let accounts = process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, updated),
                (authority, auth_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: authority, is_signer: true, is_writable: false },
            ],
            Ok(()),
        );

        let feed = get_price_feed_data(accounts[0].data()).unwrap();
        assert!(!feed.is_valid);
        // Price data is preserved
        assert_eq!(feed.price, 1_000);
    }

    #[test]
    fn test_invalidate_feed_wrong_authority() {
        use crate::oracle_instruction::invalidate_feed;

        let feed_pubkey = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let wrong_authority = Pubkey::new_unique();

        let updated = init_and_update_feed(feed_pubkey, authority, 1_000, 5, 1_000_000);
        let wrong_auth_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let ix = invalidate_feed(&feed_pubkey, &wrong_authority);
        process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, updated),
                (wrong_authority, wrong_auth_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: wrong_authority, is_signer: true, is_writable: false },
            ],
            Err(InstructionError::MissingRequiredSignature),
        );
    }

    // ---- CloseFeed --------------------------------------------------------------

    #[test]
    fn test_close_feed() {
        use crate::oracle_instruction::close_feed;
        use solana_sdk::account::ReadableAccount;

        let feed_pubkey = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();

        // Give the feed some lamports so we can verify they move
        let feed_account = AccountSharedData::new(500_000, PriceFeed::max_space() as usize, &id());
        let authority_account_raw = AccountSharedData::new(0, 0, &Pubkey::default());
        let init_ix = initialize_price_feed(&feed_pubkey, &authority, -8);
        let accounts = process_instruction(
            &init_ix.data,
            vec![
                (feed_pubkey, feed_account),
                (authority, authority_account_raw.clone()),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: true, is_writable: true },
                AccountMeta { pubkey: authority, is_signer: false, is_writable: false },
            ],
            Ok(()),
        );
        let initialized_feed = accounts[0].clone();

        let recipient_account = AccountSharedData::new(100, 0, &Pubkey::default());
        let auth_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let ix = close_feed(&feed_pubkey, &authority, &recipient);
        let accounts = process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, initialized_feed),
                (authority, auth_account),
                (recipient, recipient_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: authority, is_signer: true, is_writable: false },
                AccountMeta { pubkey: recipient, is_signer: false, is_writable: true },
            ],
            Ok(()),
        );

        // Feed account lamports must be zero
        assert_eq!(accounts[0].lamports(), 0);
        // Recipient received the 500_000 lamports
        assert_eq!(accounts[2].lamports(), 100 + 500_000);
    }

    #[test]
    fn test_close_feed_wrong_authority() {
        use crate::oracle_instruction::close_feed;

        let feed_pubkey = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let wrong_authority = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();

        let initialized = init_feed(feed_pubkey, authority, -8);
        let wrong_auth_account = AccountSharedData::new(0, 0, &Pubkey::default());
        let recipient_account = AccountSharedData::new(0, 0, &Pubkey::default());

        let ix = close_feed(&feed_pubkey, &wrong_authority, &recipient);
        process_instruction(
            &ix.data,
            vec![
                (feed_pubkey, initialized),
                (wrong_authority, wrong_auth_account),
                (recipient, recipient_account),
            ],
            vec![
                AccountMeta { pubkey: feed_pubkey, is_signer: false, is_writable: true },
                AccountMeta { pubkey: wrong_authority, is_signer: true, is_writable: false },
                AccountMeta { pubkey: recipient, is_signer: false, is_writable: true },
            ],
            Err(InstructionError::MissingRequiredSignature),
        );
    }

    // ---- PriceFeed::is_stale() --------------------------------------------------

    #[test]
    fn test_is_stale() {
        let feed = PriceFeed {
            is_valid: true,
            timestamp: 1_000,
            ..PriceFeed::default()
        };
        // Fresh: current=1_060, max_age=60 → age=60, not stale
        assert!(!feed.is_stale(1_060, 60));
        // Stale: current=1_061, max_age=60 → age=61, stale
        assert!(feed.is_stale(1_061, 60));
    }

    #[test]
    fn test_is_stale_invalid_feed() {
        // An invalid feed is always stale regardless of timestamp
        let feed = PriceFeed {
            is_valid: false,
            timestamp: 1_000,
            ..PriceFeed::default()
        };
        assert!(feed.is_stale(1_000, 9999));
    }

    // ---- PriceFeed::get_price() -------------------------------------------------

    #[test]
    fn test_get_price_valid() {
        let feed = PriceFeed {
            is_valid: true,
            price: 4_200_000_000,
            confidence: 50_000,
            ..PriceFeed::default()
        };
        assert_eq!(feed.get_price(), Some((4_200_000_000, 50_000)));
    }

    #[test]
    fn test_get_price_invalid() {
        // An invalid feed returns None
        let feed = PriceFeed {
            is_valid: false,
            price: 4_200_000_000,
            confidence: 50_000,
            ..PriceFeed::default()
        };
        assert_eq!(feed.get_price(), None);
    }

    #[test]
    fn test_get_price_after_update() {
        // Verify get_price() returns Some only after a successful UpdatePrice
        let feed_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();

        let initialized = init_feed(feed_pubkey, authority_pubkey, -8);
        // Feed was just initialized: is_valid == false
        let feed = get_price_feed_data(initialized.data()).unwrap();
        assert_eq!(feed.get_price(), None);

        // After an UpdatePrice the feed becomes valid
        let updated =
            init_and_update_feed(feed_pubkey, authority_pubkey, 9_999, 1_000, 1_700_000_000);
        let feed = get_price_feed_data(updated.data()).unwrap();
        assert_eq!(feed.get_price(), Some((9_999, 1_000)));
    }
}

