//! Oracle Adapter program processor

use {
    crate::{oracle_instruction::OracleInstruction, PriceFeed},
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
                return Err(InstructionError::InvalidAccountOwner);
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
                InstructionError::InvalidAccountData
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(InstructionError::AccountDataTooSmall);
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
                return Err(InstructionError::InvalidAccountOwner);
            }

            let mut feed: PriceFeed =
                deserialize(price_feed_account.get_data()).map_err(|err| {
                    ic_msg!(
                        invoke_context,
                        "UpdatePrice: failed to deserialize price feed: {}",
                        err
                    );
                    InstructionError::InvalidAccountData
                })?;
            drop(price_feed_account);

            if feed.authority != authority_key {
                ic_msg!(
                    invoke_context,
                    "UpdatePrice: signer is not the feed authority"
                );
                return Err(InstructionError::MissingRequiredSignature);
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
                InstructionError::InvalidAccountData
            })?;

            let mut price_feed_account =
                instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
            if price_feed_account.get_data().len() < serialized.len() {
                return Err(InstructionError::AccountDataTooSmall);
            }
            price_feed_account.get_data_mut()?[..serialized.len()].copy_from_slice(&serialized);
            Ok(())
        }
    }
});

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            get_price_feed_data, id,
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
}

