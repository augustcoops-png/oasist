//! Market data integration for Solana, providing price feeds from exchanges.
//!
//! # Binance
//!
//! Fetch the current SOL/USDT spot price from the Binance public REST API:
//!
//! ```no_run
//! use solana_market::binance::get_sol_price;
//!
//! let price = get_sol_price().unwrap();
//! println!("SOL/USDT price: {price}");
//! ```
//!
//! For repeated calls, reuse a shared [`reqwest::blocking::Client`] to benefit
//! from connection pooling:
//!
//! ```no_run
//! use reqwest::blocking::Client;
//! use solana_market::binance::get_sol_price_with_client;
//!
//! let client = Client::new();
//! let price = get_sol_price_with_client(&client).unwrap();
//! println!("SOL/USDT price: {price}");
//! ```

pub mod binance;
pub mod marketplace;
pub mod stats;
