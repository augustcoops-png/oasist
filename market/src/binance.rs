//! Binance public market data API integration.
//!
//! Fetches SOL/USDT spot price from the Binance REST API using the
//! `/api/v3/ticker/price` endpoint.

use {
    reqwest::blocking::Client,
    serde::Deserialize,
    std::fmt,
};

const BINANCE_TICKER_URL: &str = "https://api.binance.com/api/v3/ticker/price";

/// Errors that can occur when fetching data from the Binance API.
#[derive(Debug)]
pub enum BinanceError {
    /// An HTTP or network-level error occurred.
    Http(reqwest::Error),
    /// The API returned a non-success HTTP status code.
    ApiStatus(reqwest::StatusCode),
    /// The price string in the response could not be parsed as a number.
    ParsePrice(String),
}

impl fmt::Display for BinanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinanceError::Http(e) => write!(f, "HTTP error: {e}"),
            BinanceError::ApiStatus(s) => write!(f, "Binance API returned status {s}"),
            BinanceError::ParsePrice(s) => write!(f, "Failed to parse SOL price '{s}'"),
        }
    }
}

impl std::error::Error for BinanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BinanceError::Http(e) => Some(e),
            _ => None,
        }
    }
}

/// Response from the Binance ticker price endpoint.
#[derive(Deserialize)]
struct TickerPrice {
    #[allow(dead_code)]
    symbol: String,
    price: String,
}

/// Fetch the current SOL/USDT price from the Binance public API using the
/// provided HTTP client.
///
/// Callers that make repeated requests should create a single [`Client`] and
/// reuse it to benefit from connection pooling.
pub fn get_sol_price_with_client(client: &Client) -> Result<f64, BinanceError> {
    let response = client
        .get(BINANCE_TICKER_URL)
        .query(&[("symbol", "SOLUSDT")])
        .send()
        .map_err(BinanceError::Http)?;

    if !response.status().is_success() {
        return Err(BinanceError::ApiStatus(response.status()));
    }

    let ticker: TickerPrice = response.json().map_err(BinanceError::Http)?;

    ticker
        .price
        .parse::<f64>()
        .map_err(|_| BinanceError::ParsePrice(ticker.price))
}

/// Fetch the current SOL/USDT price from the Binance public API.
///
/// This is a convenience wrapper that creates a one-off HTTP client.  For
/// repeated calls, prefer [`get_sol_price_with_client`] with a shared
/// [`Client`] to benefit from connection pooling.
pub fn get_sol_price() -> Result<f64, BinanceError> {
    let client = Client::new();
    get_sol_price_with_client(&client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticker_price_deserialize() {
        let json = r#"{"symbol":"SOLUSDT","price":"23.45000000"}"#;
        let ticker: TickerPrice = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "SOLUSDT");
        let price: f64 = ticker.price.parse().unwrap();
        assert!((price - 23.45).abs() < 1e-6);
    }

    #[test]
    fn test_parse_price_error() {
        let err = BinanceError::ParsePrice("not_a_number".to_string());
        assert!(err.to_string().contains("not_a_number"));
    }

    #[test]
    fn test_api_status_error_display() {
        let err = BinanceError::ApiStatus(reqwest::StatusCode::TOO_MANY_REQUESTS);
        assert!(err.to_string().contains("429"));
    }
}
