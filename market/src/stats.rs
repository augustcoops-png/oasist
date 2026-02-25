//! Trade history and market statistics for the marketplace.
//!
//! Provides:
//!
//! * [`TradeRecord`] — a timestamped record of a single executed trade.
//! * [`Candle`] — an OHLCV bar that can cover any time resolution.
//! * [`TradeHistory`] — append-only log of trades with candle generation and
//!   rolling statistics.
//!
//! # Example
//!
//! ```
//! use solana_market::stats::{TradeHistory, TradeRecord};
//!
//! let mut history = TradeHistory::new();
//!
//! // Record two trades (timestamps are Unix seconds).
//! history.record(TradeRecord { timestamp: 1_000, price: 100.0, quantity: 2.0,
//!     buy_order_id: 1, sell_order_id: 2 });
//! history.record(TradeRecord { timestamp: 1_060, price: 102.0, quantity: 1.0,
//!     buy_order_id: 3, sell_order_id: 4 });
//!
//! // Build 1-minute (60-second) candles.
//! let candles = history.candles(60);
//! assert_eq!(candles.len(), 2);
//!
//! // Query rolling stats anchored at timestamp 1_060 over a 24-hour window.
//! let stats = history.stats(1_060, 86_400);
//! assert!(stats.is_some());
//! ```

// ─────────────────────────────────────────────────────────────────────────────
// TradeRecord
// ─────────────────────────────────────────────────────────────────────────────

/// A single executed trade recorded in the history.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRecord {
    /// Unix timestamp (seconds) when the trade occurred.
    pub timestamp: u64,
    /// Execution price in quote units (e.g. USDT).
    pub price: f64,
    /// Executed quantity in base units (e.g. SOL).
    pub quantity: f64,
    /// Order ID of the buy side.
    pub buy_order_id: u64,
    /// Order ID of the sell side.
    pub sell_order_id: u64,
}

impl TradeRecord {
    /// Notional value of this trade (`price × quantity`).
    #[inline]
    pub fn notional(&self) -> f64 {
        self.price * self.quantity
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Candle (OHLCV)
// ─────────────────────────────────────────────────────────────────────────────

/// An OHLCV candle covering a fixed time bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    /// Unix timestamp (seconds) of the bucket's opening edge.
    pub open_time: u64,
    /// Unix timestamp (seconds) of the bucket's closing edge (exclusive).
    pub close_time: u64,
    /// Price of the first trade in the bucket.
    pub open: f64,
    /// Highest trade price in the bucket.
    pub high: f64,
    /// Lowest trade price in the bucket.
    pub low: f64,
    /// Price of the last trade in the bucket.
    pub close: f64,
    /// Total traded volume (base units) in the bucket.
    pub volume: f64,
    /// Total quote-asset volume (`Σ price × quantity`) in the bucket.
    pub quote_volume: f64,
    /// Volume-weighted average price for the bucket (`quote_volume / volume`).
    pub vwap: f64,
    /// Number of individual trades that formed this candle.
    pub trade_count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// MarketStats
// ─────────────────────────────────────────────────────────────────────────────

/// Rolling statistics computed over a configurable time window.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketStats {
    /// Most-recent execution price.
    pub last_price: f64,
    /// Highest price within the window.
    pub high: f64,
    /// Lowest price within the window.
    pub low: f64,
    /// Total base-asset volume within the window.
    pub volume: f64,
    /// Total quote-asset (notional) volume within the window.
    pub quote_volume: f64,
    /// Volume-weighted average price across the window.
    pub vwap: f64,
    /// Number of trades within the window.
    pub trade_count: usize,
    /// Price of the first trade in the window.
    pub open_price: f64,
    /// Percentage price change: `(last_price − open_price) / open_price × 100`.
    pub price_change_pct: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// TradeHistory
// ─────────────────────────────────────────────────────────────────────────────

/// Append-only log of executed trades with candle and statistics generation.
///
/// Trades should be appended in non-decreasing `timestamp` order for correct
/// results, matching how a live order book emits them.
#[derive(Debug, Default)]
pub struct TradeHistory {
    records: Vec<TradeRecord>,
}

impl TradeHistory {
    /// Create an empty trade history.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Append a [`TradeRecord`].
    pub fn record(&mut self, trade: TradeRecord) {
        self.records.push(trade);
    }

    /// Number of trades recorded.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` if no trades have been recorded.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Read-only slice of all recorded trades.
    pub fn records(&self) -> &[TradeRecord] {
        &self.records
    }

    /// Most-recently recorded trade, or `None` if the history is empty.
    pub fn last_trade(&self) -> Option<&TradeRecord> {
        self.records.last()
    }

    /// Last execution price, or `None` if no trades have been recorded.
    pub fn last_price(&self) -> Option<f64> {
        self.records.last().map(|r| r.price)
    }

    // ── Candle generation ──────────────────────────────────────────────────

    /// Build OHLCV candles by dividing the trade history into buckets of
    /// `bucket_secs` seconds.
    ///
    /// Empty time buckets between trades are **not** emitted.  The returned
    /// vec is sorted by `open_time` ascending.
    ///
    /// Returns an empty vec if there are no trades or `bucket_secs == 0`.
    pub fn candles(&self, bucket_secs: u64) -> Vec<Candle> {
        if self.records.is_empty() || bucket_secs == 0 {
            return Vec::new();
        }

        let mut result: Vec<Candle> = Vec::new();

        for record in &self.records {
            let bucket_start = (record.timestamp / bucket_secs) * bucket_secs;
            let bucket_end = bucket_start + bucket_secs;

            if let Some(candle) = result.last_mut() {
                if candle.open_time == bucket_start {
                    // Extend the existing candle.
                    if record.price > candle.high {
                        candle.high = record.price;
                    }
                    if record.price < candle.low {
                        candle.low = record.price;
                    }
                    candle.close = record.price;
                    candle.volume += record.quantity;
                    candle.quote_volume += record.notional();
                    candle.vwap = candle.quote_volume / candle.volume;
                    candle.trade_count += 1;
                    continue;
                }
            }

            // Start a new candle.
            result.push(Candle {
                open_time: bucket_start,
                close_time: bucket_end,
                open: record.price,
                high: record.price,
                low: record.price,
                close: record.price,
                volume: record.quantity,
                quote_volume: record.notional(),
                vwap: record.price,
                trade_count: 1,
            });
        }

        result
    }

    // ── Rolling window statistics ──────────────────────────────────────────

    /// Compute market statistics over the window `[anchor_ts − window_secs,
    /// anchor_ts]`.
    ///
    /// Returns `None` if no trades fall within the window.
    pub fn stats(&self, anchor_ts: u64, window_secs: u64) -> Option<MarketStats> {
        let since = anchor_ts.saturating_sub(window_secs);
        let window: Vec<&TradeRecord> = self
            .records
            .iter()
            .filter(|r| r.timestamp >= since && r.timestamp <= anchor_ts)
            .collect();

        if window.is_empty() {
            return None;
        }

        let open_price = window[0].price;
        let last_price = window[window.len() - 1].price;

        let mut high = f64::NEG_INFINITY;
        let mut low = f64::INFINITY;
        let mut volume = 0.0_f64;
        let mut quote_volume = 0.0_f64;

        for r in &window {
            if r.price > high {
                high = r.price;
            }
            if r.price < low {
                low = r.price;
            }
            volume += r.quantity;
            quote_volume += r.notional();
        }

        let vwap = if volume > 0.0 {
            quote_volume / volume
        } else {
            last_price
        };

        let price_change_pct = if open_price != 0.0 {
            (last_price - open_price) / open_price * 100.0
        } else {
            0.0
        };

        Some(MarketStats {
            last_price,
            high,
            low,
            volume,
            quote_volume,
            vwap,
            trade_count: window.len(),
            open_price,
            price_change_pct,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn record(ts: u64, price: f64, qty: f64) -> TradeRecord {
        TradeRecord {
            timestamp: ts,
            price,
            quantity: qty,
            buy_order_id: 0,
            sell_order_id: 0,
        }
    }

    // ── TradeRecord ────────────────────────────────────────────────────────

    #[test]
    fn trade_record_notional() {
        let r = record(0, 100.0, 2.5);
        assert!((r.notional() - 250.0).abs() < 1e-9);
    }

    // ── TradeHistory basics ────────────────────────────────────────────────

    #[test]
    fn empty_history() {
        let h = TradeHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert!(h.last_price().is_none());
        assert!(h.last_trade().is_none());
    }

    #[test]
    fn record_and_retrieve() {
        let mut h = TradeHistory::new();
        h.record(record(1000, 100.0, 1.0));
        h.record(record(1001, 101.0, 2.0));
        assert_eq!(h.len(), 2);
        assert_eq!(h.last_price(), Some(101.0));
    }

    // ── Candle generation ──────────────────────────────────────────────────

    #[test]
    fn single_trade_makes_one_candle() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        let candles = h.candles(60);
        assert_eq!(candles.len(), 1);
        let c = &candles[0];
        assert_eq!(c.open_time, 0);
        assert_eq!(c.close_time, 60);
        assert!((c.open - 100.0).abs() < 1e-9);
        assert!((c.high - 100.0).abs() < 1e-9);
        assert!((c.low - 100.0).abs() < 1e-9);
        assert!((c.close - 100.0).abs() < 1e-9);
        assert!((c.volume - 1.0).abs() < 1e-9);
        assert!((c.vwap - 100.0).abs() < 1e-9);
        assert_eq!(c.trade_count, 1);
    }

    #[test]
    fn multiple_trades_same_bucket_ohlcv() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        h.record(record(30, 105.0, 2.0));
        h.record(record(59, 98.0, 1.0));
        let candles = h.candles(60);
        assert_eq!(candles.len(), 1);
        let c = &candles[0];
        assert!((c.open - 100.0).abs() < 1e-9);
        assert!((c.high - 105.0).abs() < 1e-9);
        assert!((c.low - 98.0).abs() < 1e-9);
        assert!((c.close - 98.0).abs() < 1e-9);
        assert!((c.volume - 4.0).abs() < 1e-9); // 1+2+1
        // quote_volume = 100*1 + 105*2 + 98*1 = 408  → vwap = 408/4 = 102
        assert!((c.quote_volume - 408.0).abs() < 1e-9);
        assert!((c.vwap - 102.0).abs() < 1e-9);
        assert_eq!(c.trade_count, 3);
    }

    #[test]
    fn trades_in_separate_buckets() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        h.record(record(60, 110.0, 1.0));
        let candles = h.candles(60);
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].open_time, 0);
        assert_eq!(candles[1].open_time, 60);
    }

    #[test]
    fn empty_history_returns_no_candles() {
        let h = TradeHistory::new();
        assert!(h.candles(60).is_empty());
    }

    #[test]
    fn zero_bucket_returns_no_candles() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        assert!(h.candles(0).is_empty());
    }

    #[test]
    fn candle_open_time_aligns_to_bucket_boundary() {
        let mut h = TradeHistory::new();
        // t=75 → bucket [60, 120)
        h.record(record(75, 100.0, 1.0));
        let candles = h.candles(60);
        assert_eq!(candles[0].open_time, 60);
        assert_eq!(candles[0].close_time, 120);
    }

    #[test]
    fn candle_quote_volume_and_vwap() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 2.0)); // notional 200
        h.record(record(10, 200.0, 1.0)); // notional 200  → total 400, vol 3
        let candles = h.candles(60);
        let c = &candles[0];
        assert!((c.quote_volume - 400.0).abs() < 1e-9);
        assert!((c.vwap - 400.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn three_buckets_correct_boundaries() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        h.record(record(60, 110.0, 1.0));
        h.record(record(120, 120.0, 1.0));
        let candles = h.candles(60);
        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].open_time, 0);
        assert_eq!(candles[1].open_time, 60);
        assert_eq!(candles[2].open_time, 120);
    }

    // ── MarketStats ────────────────────────────────────────────────────────

    #[test]
    fn stats_no_trades_returns_none() {
        let h = TradeHistory::new();
        assert!(h.stats(1000, 86_400).is_none());
    }

    #[test]
    fn stats_outside_window_returns_none() {
        let mut h = TradeHistory::new();
        h.record(record(1000, 100.0, 1.0));
        // Window [1900, 2000] — trade at 1000 is outside.
        assert!(h.stats(2000, 100).is_none());
    }

    #[test]
    fn stats_single_trade() {
        let mut h = TradeHistory::new();
        h.record(record(1000, 100.0, 2.0));
        let s = h.stats(1000, 3600).unwrap();
        assert!((s.last_price - 100.0).abs() < 1e-9);
        assert!((s.high - 100.0).abs() < 1e-9);
        assert!((s.low - 100.0).abs() < 1e-9);
        assert!((s.volume - 2.0).abs() < 1e-9);
        assert!((s.quote_volume - 200.0).abs() < 1e-9);
        assert!((s.vwap - 100.0).abs() < 1e-9);
        assert_eq!(s.trade_count, 1);
        assert!((s.price_change_pct - 0.0).abs() < 1e-9);
    }

    #[test]
    fn stats_high_low_vwap() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));   // notional 100
        h.record(record(100, 120.0, 2.0)); // notional 240
        h.record(record(200, 90.0, 3.0));  // notional 270
        let s = h.stats(200, 300).unwrap();
        assert!((s.high - 120.0).abs() < 1e-9);
        assert!((s.low - 90.0).abs() < 1e-9);
        // volume = 6, quote_volume = 610
        assert!((s.volume - 6.0).abs() < 1e-9);
        assert!((s.quote_volume - 610.0).abs() < 1e-9);
        assert!((s.vwap - 610.0 / 6.0).abs() < 1e-9);
        assert_eq!(s.trade_count, 3);
    }

    #[test]
    fn stats_price_change_pct_positive() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        h.record(record(1, 110.0, 1.0));
        let s = h.stats(1, 10).unwrap();
        assert!((s.price_change_pct - 10.0).abs() < 1e-6);
    }

    #[test]
    fn stats_price_change_pct_negative() {
        let mut h = TradeHistory::new();
        h.record(record(0, 100.0, 1.0));
        h.record(record(1, 95.0, 1.0));
        let s = h.stats(1, 10).unwrap();
        assert!((s.price_change_pct - (-5.0)).abs() < 1e-6);
    }

    #[test]
    fn stats_window_excludes_old_trades() {
        let mut h = TradeHistory::new();
        h.record(record(0, 50.0, 1.0));    // outside window [100, 200]
        h.record(record(101, 100.0, 2.0)); // inside
        h.record(record(200, 110.0, 1.0)); // inside
        let s = h.stats(200, 100).unwrap();
        assert!((s.open_price - 100.0).abs() < 1e-9);
        assert_eq!(s.trade_count, 2);
        assert!((s.low - 100.0).abs() < 1e-9);
    }

    #[test]
    fn stats_anchor_is_inclusive() {
        let mut h = TradeHistory::new();
        h.record(record(100, 100.0, 1.0));
        // window_secs = 0 → window [100, 100] — exact match is included.
        let s = h.stats(100, 0).unwrap();
        assert_eq!(s.trade_count, 1);
    }

    #[test]
    fn stats_open_and_last_price_correct() {
        let mut h = TradeHistory::new();
        h.record(record(0, 80.0, 1.0));
        h.record(record(50, 90.0, 1.0));
        h.record(record(100, 100.0, 1.0));
        let s = h.stats(100, 200).unwrap();
        assert!((s.open_price - 80.0).abs() < 1e-9);
        assert!((s.last_price - 100.0).abs() < 1e-9);
        // +25 %
        assert!((s.price_change_pct - 25.0).abs() < 1e-6);
    }
}
