//! Marketplace protocol for cryptocurrency trading.
//!
//! Implements a **central limit order book (CLOB)** with a price-time-priority
//! matching engine.  Both limit and market orders are supported.
//!
//! # Quick start
//!
//! ```
//! use solana_market::marketplace::{MatchingEngine, OrderSide, OrderType, OrderStatus};
//!
//! let mut engine = MatchingEngine::new();
//!
//! // Post a resting limit sell at 100.00 for 2.0 units
//! let (ask, _) = engine.submit_order(OrderSide::Sell, OrderType::Limit, Some(100.0), 2.0);
//! assert_eq!(ask.status, OrderStatus::Open);
//!
//! // Incoming limit buy at 100.00 for 1.0 unit — matches immediately
//! let (bid, trades) = engine.submit_order(OrderSide::Buy, OrderType::Limit, Some(100.0), 1.0);
//! assert_eq!(trades.len(), 1);
//! assert_eq!(bid.status, OrderStatus::Filled);
//! ```

use std::{
    cmp::Reverse,
    collections::{BTreeMap, VecDeque},
};

// ─────────────────────────────────────────────────────────────────────────────
// Primitive types
// ─────────────────────────────────────────────────────────────────────────────

/// Price in micro-units (1 unit = 0.000001 quote currency, e.g. USDT).
pub type Price = u64;

/// Quantity in micro-units (1 unit = 0.000001 base token, e.g. SOL).
pub type Quantity = u64;

/// Unique order identifier issued by the [`MatchingEngine`].
pub type OrderId = u64;

/// Micro-units per one unit of price.
pub const PRICE_SCALE: f64 = 1_000_000.0;

/// Micro-units per one unit of quantity.
pub const QUANTITY_SCALE: f64 = 1_000_000.0;

/// Convert a floating-point price to micro-units, rounding to the nearest unit.
pub fn to_price(f: f64) -> Price {
    (f * PRICE_SCALE).round() as u64
}

/// Convert a micro-unit price back to a floating-point value.
pub fn from_price(p: Price) -> f64 {
    p as f64 / PRICE_SCALE
}

/// Convert a floating-point quantity to micro-units, rounding to the nearest unit.
pub fn to_quantity(f: f64) -> Quantity {
    (f * QUANTITY_SCALE).round() as u64
}

/// Convert a micro-unit quantity back to a floating-point value.
pub fn from_quantity(q: Quantity) -> f64 {
    q as f64 / QUANTITY_SCALE
}

// ─────────────────────────────────────────────────────────────────────────────
// Order model
// ─────────────────────────────────────────────────────────────────────────────

/// Which side of the market an order is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    /// A buy (bid) order.
    Buy,
    /// A sell (ask) order.
    Sell,
}

/// Determines how an order is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Execute immediately at the best available price; cancel any unfilled
    /// remainder.
    Market,
    /// Execute at the specified price or better; rest on the order book if
    /// not immediately matched.
    Limit,
}

/// Lifecycle state of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    /// No quantity has been filled yet.
    Open,
    /// Some — but not all — quantity has been filled.
    PartiallyFilled,
    /// All requested quantity has been executed.
    Filled,
    /// The order was cancelled (explicit cancellation or an unfillable market
    /// order).
    Cancelled,
}

/// A single buy or sell order.
#[derive(Debug, Clone)]
pub struct Order {
    /// Unique identifier assigned by the [`MatchingEngine`].
    pub id: OrderId,
    /// Buy or sell.
    pub side: OrderSide,
    /// Limit or market.
    pub order_type: OrderType,
    /// Limit price in micro-units; `None` for market orders.
    pub price: Option<Price>,
    /// Total requested quantity in micro-units.
    pub quantity: Quantity,
    /// Quantity already executed, in micro-units.
    pub filled_quantity: Quantity,
    /// Current lifecycle status.
    pub status: OrderStatus,
}

impl Order {
    /// Remaining unfilled quantity in micro-units.
    #[inline]
    pub fn remaining(&self) -> Quantity {
        self.quantity.saturating_sub(self.filled_quantity)
    }

    /// `true` if this order has been fully executed.
    #[inline]
    pub fn is_filled(&self) -> bool {
        self.remaining() == 0
    }

    /// Limit price as a floating-point value, or `None` for market orders.
    pub fn price_f64(&self) -> Option<f64> {
        self.price.map(from_price)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Trade
// ─────────────────────────────────────────────────────────────────────────────

/// A completed trade resulting from the matching of a buy order against a sell
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    /// Identifier of the buy-side order.
    pub buy_order_id: OrderId,
    /// Identifier of the sell-side order.
    pub sell_order_id: OrderId,
    /// Execution price in micro-units.
    pub price: Price,
    /// Executed quantity in micro-units.
    pub quantity: Quantity,
}

impl Trade {
    /// Execution price as a floating-point value.
    pub fn price_f64(&self) -> f64 {
        from_price(self.price)
    }

    /// Executed quantity as a floating-point value.
    pub fn quantity_f64(&self) -> f64 {
        from_quantity(self.quantity)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Order book
// ─────────────────────────────────────────────────────────────────────────────

/// A central limit order book (CLOB).
///
/// Bids are stored highest-price-first; asks lowest-price-first.  Within each
/// price level orders are kept in FIFO arrival order.
pub struct OrderBook {
    /// Bids — `Reverse` key flips `BTreeMap`'s ascending order to descending.
    bids: BTreeMap<Reverse<Price>, VecDeque<Order>>,
    /// Asks — ascending order by price.
    asks: BTreeMap<Price, VecDeque<Order>>,
}

impl OrderBook {
    /// Create an empty order book.
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Best (highest) bid price, or `None` if the bid side is empty.
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next().map(|Reverse(p)| from_price(*p))
    }

    /// Best (lowest) ask price, or `None` if the ask side is empty.
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().copied().map(from_price)
    }

    /// Bid–ask spread, or `None` if either side of the book is empty.
    pub fn spread(&self) -> Option<f64> {
        let ask = self.asks.keys().next().copied()?;
        let Reverse(bid) = self.bids.keys().next().copied()?;
        Some(from_price(ask.saturating_sub(bid)))
    }

    /// Mid-price (arithmetic mean of best bid and best ask), or `None` if
    /// either side of the book is empty.
    pub fn mid_price(&self) -> Option<f64> {
        let ask = self.asks.keys().next().copied()? as f64;
        let Reverse(bid) = self.bids.keys().next().copied()?;
        Some((ask + bid as f64) / 2.0 / PRICE_SCALE)
    }

    /// All bid price levels as `(price, total_quantity)` pairs, sorted
    /// highest-price first.
    pub fn bid_levels(&self) -> Vec<(f64, f64)> {
        self.bids
            .iter()
            .map(|(Reverse(price), orders)| {
                let qty: Quantity = orders.iter().map(|o| o.remaining()).sum();
                (from_price(*price), from_quantity(qty))
            })
            .collect()
    }

    /// All ask price levels as `(price, total_quantity)` pairs, sorted
    /// lowest-price first.
    pub fn ask_levels(&self) -> Vec<(f64, f64)> {
        self.asks
            .iter()
            .map(|(price, orders)| {
                let qty: Quantity = orders.iter().map(|o| o.remaining()).sum();
                (from_price(*price), from_quantity(qty))
            })
            .collect()
    }

    fn add_bid(&mut self, order: Order) {
        if let Some(price) = order.price {
            self.bids.entry(Reverse(price)).or_default().push_back(order);
        }
    }

    fn add_ask(&mut self, order: Order) {
        if let Some(price) = order.price {
            self.asks.entry(price).or_default().push_back(order);
        }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Matching engine
// ─────────────────────────────────────────────────────────────────────────────

/// Price-time-priority (FIFO) matching engine.
///
/// Accepts incoming orders, matches them against the resting order book, and
/// returns the resulting [`Trade`]s.  Unmatched limit-order remainder is rested
/// on the book; unmatched market-order remainder is cancelled.
pub struct MatchingEngine {
    /// The underlying order book.  Exposed for read-only queries.
    pub order_book: OrderBook,
    next_id: OrderId,
}

impl MatchingEngine {
    /// Create a new engine with an empty order book.
    pub fn new() -> Self {
        Self {
            order_book: OrderBook::new(),
            next_id: 1,
        }
    }

    /// Submit an order to the engine.
    ///
    /// Returns a `(order_snapshot, trades)` tuple where `order_snapshot`
    /// reflects the order state immediately after initial matching.
    ///
    /// | Parameter    | Description |
    /// |---|---|
    /// | `side`       | [`OrderSide::Buy`] or [`OrderSide::Sell`] |
    /// | `order_type` | [`OrderType::Limit`] or [`OrderType::Market`] |
    /// | `price`      | Limit price in quote units; `None` for market orders |
    /// | `quantity`   | Requested quantity in base units |
    pub fn submit_order(
        &mut self,
        side: OrderSide,
        order_type: OrderType,
        price: Option<f64>,
        quantity: f64,
    ) -> (Order, Vec<Trade>) {
        let id = self.next_id;
        self.next_id += 1;

        let mut order = Order {
            id,
            side,
            order_type,
            price: price.map(to_price),
            quantity: to_quantity(quantity),
            filled_quantity: 0,
            status: OrderStatus::Open,
        };

        let trades = match side {
            OrderSide::Buy => self.match_buy(&mut order),
            OrderSide::Sell => self.match_sell(&mut order),
        };

        if order.remaining() > 0 {
            match order_type {
                OrderType::Limit => match side {
                    OrderSide::Buy => self.order_book.add_bid(order.clone()),
                    OrderSide::Sell => self.order_book.add_ask(order.clone()),
                },
                // Market orders with unmatched remainder are cancelled.
                OrderType::Market => {
                    order.status = OrderStatus::Cancelled;
                }
            }
        }

        (order, trades)
    }

    /// Cancel a resting order by its `order_id` and `side`.
    ///
    /// Returns `true` if the order was found and removed from the book.
    pub fn cancel_order(&mut self, order_id: OrderId, side: OrderSide) -> bool {
        match side {
            OrderSide::Buy => {
                let mut remove_key: Option<Reverse<Price>> = None;
                let mut found = false;
                for (key, queue) in &mut self.order_book.bids {
                    if let Some(pos) = queue.iter().position(|o| o.id == order_id) {
                        queue.remove(pos);
                        found = true;
                        if queue.is_empty() {
                            remove_key = Some(*key);
                        }
                        break;
                    }
                }
                if let Some(k) = remove_key {
                    self.order_book.bids.remove(&k);
                }
                found
            }
            OrderSide::Sell => {
                let mut remove_key: Option<Price> = None;
                let mut found = false;
                for (key, queue) in &mut self.order_book.asks {
                    if let Some(pos) = queue.iter().position(|o| o.id == order_id) {
                        queue.remove(pos);
                        found = true;
                        if queue.is_empty() {
                            remove_key = Some(*key);
                        }
                        break;
                    }
                }
                if let Some(k) = remove_key {
                    self.order_book.asks.remove(&k);
                }
                found
            }
        }
    }

    // ── Internal matching helpers ──────────────────────────────────────────

    /// Match an incoming buy order against resting asks (lowest ask first).
    fn match_buy(&mut self, incoming: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut empty_keys: Vec<Price> = Vec::new();

        for (ask_price, queue) in &mut self.order_book.asks {
            if incoming.remaining() == 0 {
                break;
            }
            // Limit order: only match at or below the limit price.
            if let Some(limit) = incoming.price {
                if *ask_price > limit {
                    break;
                }
            }
            let exec_price = *ask_price;

            loop {
                if incoming.remaining() == 0 {
                    break;
                }
                let (exec_qty, resting_id, resting_done) = {
                    let Some(resting) = queue.front_mut() else {
                        break;
                    };
                    let q = incoming.remaining().min(resting.remaining());
                    incoming.filled_quantity += q;
                    resting.filled_quantity += q;
                    let done = resting.remaining() == 0;
                    resting.status = if done {
                        OrderStatus::Filled
                    } else {
                        OrderStatus::PartiallyFilled
                    };
                    (q, resting.id, done)
                };

                trades.push(Trade {
                    buy_order_id: incoming.id,
                    sell_order_id: resting_id,
                    price: exec_price,
                    quantity: exec_qty,
                });

                if resting_done {
                    queue.pop_front();
                } else {
                    break;
                }
            }

            if queue.is_empty() {
                empty_keys.push(exec_price);
            }
        }

        for k in empty_keys {
            self.order_book.asks.remove(&k);
        }

        Self::update_status(incoming);
        trades
    }

    /// Match an incoming sell order against resting bids (highest bid first).
    fn match_sell(&mut self, incoming: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut empty_keys: Vec<Reverse<Price>> = Vec::new();

        for (bid_key, queue) in &mut self.order_book.bids {
            if incoming.remaining() == 0 {
                break;
            }
            let Reverse(bid_price) = *bid_key;
            // Limit order: only match at or above the limit price.
            if let Some(limit) = incoming.price {
                if bid_price < limit {
                    break;
                }
            }
            let exec_price = bid_price;

            loop {
                if incoming.remaining() == 0 {
                    break;
                }
                let (exec_qty, resting_id, resting_done) = {
                    let Some(resting) = queue.front_mut() else {
                        break;
                    };
                    let q = incoming.remaining().min(resting.remaining());
                    incoming.filled_quantity += q;
                    resting.filled_quantity += q;
                    let done = resting.remaining() == 0;
                    resting.status = if done {
                        OrderStatus::Filled
                    } else {
                        OrderStatus::PartiallyFilled
                    };
                    (q, resting.id, done)
                };

                trades.push(Trade {
                    buy_order_id: resting_id,
                    sell_order_id: incoming.id,
                    price: exec_price,
                    quantity: exec_qty,
                });

                if resting_done {
                    queue.pop_front();
                } else {
                    break;
                }
            }

            if queue.is_empty() {
                empty_keys.push(*bid_key);
            }
        }

        for k in empty_keys {
            self.order_book.bids.remove(&k);
        }

        Self::update_status(incoming);
        trades
    }

    /// Update the status of `order` after matching has run.
    fn update_status(order: &mut Order) {
        if order.filled_quantity == 0 {
            return;
        }
        order.status = if order.remaining() == 0 {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ────────────────────────────────────────────────────────────

    fn limit_buy(engine: &mut MatchingEngine, price: f64, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Buy, OrderType::Limit, Some(price), qty)
    }

    fn limit_sell(engine: &mut MatchingEngine, price: f64, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Sell, OrderType::Limit, Some(price), qty)
    }

    fn market_buy(engine: &mut MatchingEngine, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Buy, OrderType::Market, None, qty)
    }

    fn market_sell(engine: &mut MatchingEngine, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Sell, OrderType::Market, None, qty)
    }

    // ── Basic order placement ──────────────────────────────────────────────

    #[test]
    fn limit_buy_rests_on_empty_book() {
        let mut e = MatchingEngine::new();
        let (order, trades) = limit_buy(&mut e, 99.0, 1.0);
        assert_eq!(trades.len(), 0);
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(e.order_book.best_bid(), Some(99.0));
        assert!(e.order_book.best_ask().is_none());
    }

    #[test]
    fn limit_sell_rests_on_empty_book() {
        let mut e = MatchingEngine::new();
        let (order, trades) = limit_sell(&mut e, 101.0, 1.0);
        assert_eq!(trades.len(), 0);
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(e.order_book.best_ask(), Some(101.0));
        assert!(e.order_book.best_bid().is_none());
    }

    // ── No crossing ────────────────────────────────────────────────────────

    #[test]
    fn no_match_when_bid_below_ask() {
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 101.0, 1.0);
        let (bid, trades) = limit_buy(&mut e, 99.0, 1.0);
        assert_eq!(trades.len(), 0);
        assert_eq!(bid.status, OrderStatus::Open);
        // Both orders are now on the book.
        assert_eq!(e.order_book.best_bid(), Some(99.0));
        assert_eq!(e.order_book.best_ask(), Some(101.0));
    }

    // ── Full match ─────────────────────────────────────────────────────────

    #[test]
    fn full_match_limit_buy_into_ask() {
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 100.0, 1.0);
        let (bid, trades) = limit_buy(&mut e, 100.0, 1.0);
        assert_eq!(trades.len(), 1);
        assert_eq!(bid.status, OrderStatus::Filled);
        assert!((trades[0].price_f64() - 100.0).abs() < 1e-4);
        assert!((trades[0].quantity_f64() - 1.0).abs() < 1e-4);
        // Book should be empty after full match.
        assert!(e.order_book.best_bid().is_none());
        assert!(e.order_book.best_ask().is_none());
    }

    #[test]
    fn full_match_limit_sell_into_bid() {
        let mut e = MatchingEngine::new();
        limit_buy(&mut e, 100.0, 1.0);
        let (ask, trades) = limit_sell(&mut e, 100.0, 1.0);
        assert_eq!(trades.len(), 1);
        assert_eq!(ask.status, OrderStatus::Filled);
        assert!((trades[0].price_f64() - 100.0).abs() < 1e-4);
    }

    // ── Partial fills ──────────────────────────────────────────────────────

    #[test]
    fn incoming_partially_filled_remainder_rests() {
        // Buy 2, ask only has 1 → 1 matched, 1 rests as bid.
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 100.0, 1.0);
        let (bid, trades) = limit_buy(&mut e, 100.0, 2.0);
        assert_eq!(trades.len(), 1);
        assert_eq!(bid.status, OrderStatus::PartiallyFilled);
        // Remaining 1 unit should be on the bid book.
        assert_eq!(e.order_book.best_bid(), Some(100.0));
        let levels = e.order_book.bid_levels();
        assert!((levels[0].1 - 1.0).abs() < 1e-4, "expected 1.0 remaining");
    }

    #[test]
    fn resting_order_partially_filled() {
        // Ask for 2, buy only 1 → ask is 1 filled, 1 remains.
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 100.0, 2.0);
        let (bid, trades) = limit_buy(&mut e, 100.0, 1.0);
        assert_eq!(trades.len(), 1);
        assert_eq!(bid.status, OrderStatus::Filled);
        // The ask should have 1 unit remaining on the book.
        assert_eq!(e.order_book.best_ask(), Some(100.0));
        let levels = e.order_book.ask_levels();
        assert!((levels[0].1 - 1.0).abs() < 1e-4, "expected 1.0 remaining");
    }

    // ── Market orders ──────────────────────────────────────────────────────

    #[test]
    fn market_buy_fills_from_lowest_ask() {
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 102.0, 5.0);
        limit_sell(&mut e, 100.0, 5.0);
        limit_sell(&mut e, 101.0, 5.0);
        // Market buy should hit the 100.0 ask first.
        let (order, trades) = market_buy(&mut e, 3.0);
        assert_eq!(trades.len(), 1);
        assert!((trades[0].price_f64() - 100.0).abs() < 1e-4);
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn market_sell_fills_from_highest_bid() {
        let mut e = MatchingEngine::new();
        limit_buy(&mut e, 98.0, 5.0);
        limit_buy(&mut e, 100.0, 5.0);
        limit_buy(&mut e, 99.0, 5.0);
        // Market sell should hit the 100.0 bid first.
        let (order, trades) = market_sell(&mut e, 3.0);
        assert_eq!(trades.len(), 1);
        assert!((trades[0].price_f64() - 100.0).abs() < 1e-4);
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn market_order_cancelled_when_no_liquidity() {
        let mut e = MatchingEngine::new();
        let (order, trades) = market_buy(&mut e, 1.0);
        assert_eq!(trades.len(), 0);
        assert_eq!(order.status, OrderStatus::Cancelled);
    }

    // ── Multi-level matching ───────────────────────────────────────────────

    #[test]
    fn buy_sweeps_multiple_ask_levels() {
        let mut e = MatchingEngine::new();
        limit_sell(&mut e, 100.0, 1.0);
        limit_sell(&mut e, 101.0, 1.0);
        limit_sell(&mut e, 102.0, 1.0);
        // Buy 3 units with limit 102 — should sweep all three levels.
        let (bid, trades) = limit_buy(&mut e, 102.0, 3.0);
        assert_eq!(trades.len(), 3);
        assert_eq!(bid.status, OrderStatus::Filled);
        assert!(e.order_book.best_ask().is_none());
        // Execution prices should be ascending (price-time priority).
        let prices: Vec<f64> = trades.iter().map(|t| t.price_f64()).collect();
        assert!((prices[0] - 100.0).abs() < 1e-4);
        assert!((prices[1] - 101.0).abs() < 1e-4);
        assert!((prices[2] - 102.0).abs() < 1e-4);
    }

    // ── FIFO within a price level ──────────────────────────────────────────

    #[test]
    fn fifo_within_price_level() {
        let mut e = MatchingEngine::new();
        let (first_ask, _) = limit_sell(&mut e, 100.0, 1.0);
        let (second_ask, _) = limit_sell(&mut e, 100.0, 1.0);
        // Buy 1: should match the first (earlier) ask.
        let (_bid, trades) = limit_buy(&mut e, 100.0, 1.0);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].sell_order_id, first_ask.id);
        // Second ask still resting.
        let levels = e.order_book.ask_levels();
        assert!((levels[0].1 - 1.0).abs() < 1e-4);
        let (_bid2, trades2) = limit_buy(&mut e, 100.0, 1.0);
        assert_eq!(trades2[0].sell_order_id, second_ask.id);
    }

    // ── Order cancellation ─────────────────────────────────────────────────

    #[test]
    fn cancel_resting_bid() {
        let mut e = MatchingEngine::new();
        let (order, _) = limit_buy(&mut e, 99.0, 1.0);
        assert!(e.cancel_order(order.id, OrderSide::Buy));
        assert!(e.order_book.best_bid().is_none());
    }

    #[test]
    fn cancel_resting_ask() {
        let mut e = MatchingEngine::new();
        let (order, _) = limit_sell(&mut e, 101.0, 1.0);
        assert!(e.cancel_order(order.id, OrderSide::Sell));
        assert!(e.order_book.best_ask().is_none());
    }

    #[test]
    fn cancel_nonexistent_order_returns_false() {
        let mut e = MatchingEngine::new();
        assert!(!e.cancel_order(999, OrderSide::Buy));
    }

    #[test]
    fn cancel_one_of_two_at_same_price_level() {
        let mut e = MatchingEngine::new();
        let (o1, _) = limit_buy(&mut e, 99.0, 1.0);
        let (_o2, _) = limit_buy(&mut e, 99.0, 2.0);
        assert!(e.cancel_order(o1.id, OrderSide::Buy));
        // Level still exists with the second order's quantity.
        let levels = e.order_book.bid_levels();
        assert_eq!(levels.len(), 1);
        assert!((levels[0].1 - 2.0).abs() < 1e-4);
    }

    // ── Order book queries ─────────────────────────────────────────────────

    #[test]
    fn spread_and_mid_price() {
        let mut e = MatchingEngine::new();
        limit_buy(&mut e, 99.0, 1.0);
        limit_sell(&mut e, 101.0, 1.0);
        let spread = e.order_book.spread().unwrap();
        assert!((spread - 2.0).abs() < 1e-4, "spread should be 2.0");
        let mid = e.order_book.mid_price().unwrap();
        assert!((mid - 100.0).abs() < 1e-4, "mid should be 100.0");
    }

    #[test]
    fn bid_ask_levels_aggregated_correctly() {
        let mut e = MatchingEngine::new();
        limit_buy(&mut e, 99.0, 1.0);
        limit_buy(&mut e, 99.0, 2.0); // same level
        limit_buy(&mut e, 98.0, 3.0);
        limit_sell(&mut e, 101.0, 4.0);
        limit_sell(&mut e, 102.0, 5.0);

        let bids = e.order_book.bid_levels();
        assert_eq!(bids.len(), 2);
        assert!((bids[0].0 - 99.0).abs() < 1e-4);
        assert!((bids[0].1 - 3.0).abs() < 1e-4); // 1 + 2
        assert!((bids[1].0 - 98.0).abs() < 1e-4);
        assert!((bids[1].1 - 3.0).abs() < 1e-4);

        let asks = e.order_book.ask_levels();
        assert_eq!(asks.len(), 2);
        assert!((asks[0].0 - 101.0).abs() < 1e-4);
        assert!((asks[0].1 - 4.0).abs() < 1e-4);
        assert!((asks[1].0 - 102.0).abs() < 1e-4);
        assert!((asks[1].1 - 5.0).abs() < 1e-4);
    }

    #[test]
    fn spread_none_when_book_empty() {
        let e = MatchingEngine::new();
        assert!(e.order_book.spread().is_none());
        assert!(e.order_book.mid_price().is_none());
        assert!(e.order_book.best_bid().is_none());
        assert!(e.order_book.best_ask().is_none());
    }

    // ── Scale helpers ──────────────────────────────────────────────────────

    #[test]
    fn price_round_trip() {
        for val in [0.0, 1.0, 99.999999, 123.456789, 100_000.0] {
            let rounded = from_price(to_price(val));
            assert!((rounded - val).abs() < 1e-6, "{val} round-trip failed");
        }
    }

    #[test]
    fn quantity_round_trip() {
        for val in [0.0, 0.5, 1.0, 1000.0] {
            let rounded = from_quantity(to_quantity(val));
            assert!((rounded - val).abs() < 1e-6, "{val} round-trip failed");
        }
    }
}
