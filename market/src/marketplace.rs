//! Marketplace protocol for cryptocurrency trading.
//!
//! Implements a **central limit order book (CLOB)** with a price-time-priority
//! matching engine.  Both limit and market orders are supported.
//!
//! An optional **daily price band** can be attached to cap how far the market
//! may move in one session.  By default the band is ±5 %; when the market is
//! "on a roll" (a configurable streak of consecutive up-ticks) the upside cap
//! expands to 20 % while the downside protection stays at 5 %.
//!
//! # Quick start
//!
//! ```
//! use solana_market::marketplace::{MatchingEngine, OrderSide, OrderType, OrderStatus};
//!
//! let mut engine = MatchingEngine::new();
//!
//! // Post a resting limit sell at 100.00 for 2.0 units
//! let (ask, _) = engine.submit_order(OrderSide::Sell, OrderType::Limit, Some(100.0), 2.0).unwrap();
//! assert_eq!(ask.status, OrderStatus::Open);
//!
//! // Incoming limit buy at 100.00 for 1.0 unit — matches immediately
//! let (bid, trades) = engine.submit_order(OrderSide::Buy, OrderType::Limit, Some(100.0), 1.0).unwrap();
//! assert_eq!(trades.len(), 1);
//! assert_eq!(bid.status, OrderStatus::Filled);
//! ```

use std::{
    cmp::Reverse,
    collections::{BTreeMap, VecDeque},
    fmt,
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
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Errors returned by [`MatchingEngine::submit_order`].
#[derive(Debug, Clone, PartialEq)]
pub enum MarketplaceError {
    /// The limit price falls outside the session's allowed price band.
    PriceBandViolation {
        /// The submitted limit price (quote units).
        price: f64,
        /// Lower boundary of today's allowed band.
        lower_limit: f64,
        /// Upper boundary of today's allowed band.
        upper_limit: f64,
    },
}

impl fmt::Display for MarketplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketplaceError::PriceBandViolation {
                price,
                lower_limit,
                upper_limit,
            } => write!(
                f,
                "price {price:.6} is outside the allowed daily band \
                 [{lower_limit:.6}, {upper_limit:.6}]"
            ),
        }
    }
}

impl std::error::Error for MarketplaceError {}

// ─────────────────────────────────────────────────────────────────────────────
// Price band
// ─────────────────────────────────────────────────────────────────────────────

/// An asymmetric daily price-change limit attached to a [`MatchingEngine`].
///
/// * **Downside** is always capped at `max_down_pct` below the reference price
///   (default 5 %).
/// * **Normal upside** is capped at `normal_up_pct` above the reference price
///   (default 5 %).
/// * **Momentum upside** — when the market is "on a roll" (the last
///   `streak_threshold` consecutive trades were each at a higher price than the
///   previous one) — the upper cap expands to `momentum_up_pct` above the
///   reference price (default 20 %).
///
/// # Example
///
/// ```
/// use solana_market::marketplace::PriceBand;
///
/// // ±5 % band; expands to +20 % upside after 3 consecutive up-ticks.
/// let band = PriceBand::new(100.0, 5.0);
/// assert!((band.lower_limit() - 95.0).abs() < 1e-4);
/// assert!((band.upper_limit() - 105.0).abs() < 1e-4); // no streak yet
/// ```
#[derive(Debug, Clone)]
pub struct PriceBand {
    /// Reference (opening) price in micro-units.
    reference: Price,
    /// Maximum allowed downside move as a fraction (e.g. 0.05 = 5 %).
    max_down_pct: f64,
    /// Normal upside cap as a fraction (e.g. 0.05 = 5 %).
    normal_up_pct: f64,
    /// Momentum upside cap as a fraction (e.g. 0.20 = 20 %).
    momentum_up_pct: f64,
    /// Number of consecutive up-ticks required to activate the momentum cap.
    streak_threshold: usize,
    /// Rolling window of recent trade prices (micro-units), newest at back.
    recent_prices: VecDeque<Price>,
}

impl PriceBand {
    /// Create a price band with symmetric normal limits of `max_change_pct` %,
    /// a 20 % momentum upside cap, and a 3-trade streak threshold.
    ///
    /// * `reference`      — today's reference (opening) price in quote units
    /// * `max_change_pct` — normal max daily move in percent (e.g. `5.0`)
    pub fn new(reference: f64, max_change_pct: f64) -> Self {
        Self::with_config(reference, max_change_pct, max_change_pct, 20.0, 3)
    }

    /// Create a fully-configured price band.
    ///
    /// * `reference`        — today's reference price in quote units
    /// * `max_down_pct`     — maximum downside move in percent
    /// * `normal_up_pct`    — normal maximum upside move in percent
    /// * `momentum_up_pct`  — upside cap when on a roll, in percent
    /// * `streak_threshold` — consecutive up-ticks needed to be "on a roll"
    pub fn with_config(
        reference: f64,
        max_down_pct: f64,
        normal_up_pct: f64,
        momentum_up_pct: f64,
        streak_threshold: usize,
    ) -> Self {
        Self {
            reference: to_price(reference),
            max_down_pct: max_down_pct / 100.0,
            normal_up_pct: normal_up_pct / 100.0,
            momentum_up_pct: momentum_up_pct / 100.0,
            streak_threshold,
            recent_prices: VecDeque::new(),
        }
    }

    /// Reference (opening) price in quote units.
    pub fn reference_price(&self) -> f64 {
        from_price(self.reference)
    }

    /// Lower boundary of the allowed band (quote units).
    pub fn lower_limit(&self) -> f64 {
        from_price(self.reference) * (1.0 - self.max_down_pct)
    }

    /// Upper boundary of the allowed band (quote units).
    ///
    /// Returns the momentum cap when the market is on a roll, otherwise the
    /// normal cap.
    pub fn upper_limit(&self) -> f64 {
        let pct = if self.is_on_a_roll() {
            self.momentum_up_pct
        } else {
            self.normal_up_pct
        };
        from_price(self.reference) * (1.0 + pct)
    }

    /// `true` if `price` (quote units) falls within `[lower_limit, upper_limit]`.
    pub fn allows(&self, price: f64) -> bool {
        price >= self.lower_limit() && price <= self.upper_limit()
    }

    /// `true` when the last `streak_threshold` trades were each at a strictly
    /// higher price than the previous one.
    pub fn is_on_a_roll(&self) -> bool {
        if self.recent_prices.len() < self.streak_threshold {
            return false;
        }
        self.recent_prices
            .iter()
            .zip(self.recent_prices.iter().skip(1))
            .all(|(a, b)| b > a)
    }

    /// Record an execution price from a completed trade.
    ///
    /// Maintains a rolling window of `streak_threshold` prices so that
    /// [`is_on_a_roll`](Self::is_on_a_roll) can be evaluated efficiently.
    pub fn record_trade(&mut self, exec_price: Price) {
        self.recent_prices.push_back(exec_price);
        while self.recent_prices.len() > self.streak_threshold {
            self.recent_prices.pop_front();
        }
    }

    /// Reset the reference price for a new trading session and clear the
    /// momentum streak.
    pub fn reset(&mut self, new_reference: f64) {
        self.reference = to_price(new_reference);
        self.recent_prices.clear();
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
    price_band: Option<PriceBand>,
}

impl MatchingEngine {
    /// Create a new engine with an empty order book and no price band.
    pub fn new() -> Self {
        Self {
            order_book: OrderBook::new(),
            next_id: 1,
            price_band: None,
        }
    }

    /// Submit an order to the engine.
    ///
    /// Returns `Ok((order_snapshot, trades))` on success, where
    /// `order_snapshot` reflects the order state immediately after initial
    /// matching.  Returns `Err` if a price band is active and the limit price
    /// falls outside it.
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
    ) -> Result<(Order, Vec<Trade>), MarketplaceError> {
        // Enforce daily price band for limit orders.
        if let (OrderType::Limit, Some(limit_price)) = (order_type, price) {
            if let Some(band) = &self.price_band {
                if !band.allows(limit_price) {
                    return Err(MarketplaceError::PriceBandViolation {
                        price: limit_price,
                        lower_limit: band.lower_limit(),
                        upper_limit: band.upper_limit(),
                    });
                }
            }
        }

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

        // Record execution prices into the price band for momentum tracking.
        if let Some(band) = &mut self.price_band {
            for trade in &trades {
                band.record_trade(trade.price);
            }
        }

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

        Ok((order, trades))
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

    // ── Price band management ──────────────────────────────────────────────

    /// Attach a [`PriceBand`] built with [`PriceBand::new`] (±`max_change_pct` %
    /// normal limits, 20 % momentum upside cap, 3-trade streak threshold).
    ///
    /// * `reference`      — today's opening/reference price in quote units
    /// * `max_change_pct` — normal max daily move in percent (e.g. `5.0`)
    pub fn set_price_band(&mut self, reference: f64, max_change_pct: f64) {
        self.price_band = Some(PriceBand::new(reference, max_change_pct));
    }

    /// Attach a fully-configured [`PriceBand`].
    pub fn set_price_band_custom(&mut self, band: PriceBand) {
        self.price_band = Some(band);
    }

    /// Remove the price band so all prices are accepted.
    pub fn clear_price_band(&mut self) {
        self.price_band = None;
    }

    /// Reset the price band's reference price and clear the momentum streak,
    /// e.g. at the start of a new trading day.  No-op if no band is set.
    pub fn reset_daily_reference(&mut self, new_reference: f64) {
        if let Some(band) = &mut self.price_band {
            band.reset(new_reference);
        }
    }

    /// Read-only access to the current price band, if any.
    pub fn price_band(&self) -> Option<&PriceBand> {
        self.price_band.as_ref()
    }

    // ── Internal matching helpers ──────────────────────────────────────────
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
        engine.submit_order(OrderSide::Buy, OrderType::Limit, Some(price), qty).unwrap()
    }

    fn limit_sell(engine: &mut MatchingEngine, price: f64, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Sell, OrderType::Limit, Some(price), qty).unwrap()
    }

    fn market_buy(engine: &mut MatchingEngine, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Buy, OrderType::Market, None, qty).unwrap()
    }

    fn market_sell(engine: &mut MatchingEngine, qty: f64) -> (Order, Vec<Trade>) {
        engine.submit_order(OrderSide::Sell, OrderType::Market, None, qty).unwrap()
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

    // ── Price band — basic limits ──────────────────────────────────────────

    #[test]
    fn band_lower_and_upper_limits() {
        let band = PriceBand::new(100.0, 5.0);
        assert!((band.lower_limit() - 95.0).abs() < 1e-4);
        // No streak yet → normal 5 % cap.
        assert!((band.upper_limit() - 105.0).abs() < 1e-4);
    }

    #[test]
    fn band_accepts_price_inside_range() {
        let band = PriceBand::new(100.0, 5.0);
        assert!(band.allows(100.0));
        assert!(band.allows(95.0));
        assert!(band.allows(105.0));
    }

    #[test]
    fn band_rejects_price_above_normal_cap() {
        let band = PriceBand::new(100.0, 5.0);
        assert!(!band.allows(105.01));
        assert!(!band.allows(120.0));
    }

    #[test]
    fn band_rejects_price_below_lower_limit() {
        let band = PriceBand::new(100.0, 5.0);
        assert!(!band.allows(94.99));
        assert!(!band.allows(50.0));
    }

    // ── Engine rejects out-of-band limit orders ────────────────────────────

    #[test]
    fn engine_rejects_limit_buy_above_band() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);
        let err = e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(110.0), 1.0)
            .unwrap_err();
        match err {
            MarketplaceError::PriceBandViolation { price, .. } => {
                assert!((price - 110.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn engine_rejects_limit_sell_below_band() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);
        let err = e
            .submit_order(OrderSide::Sell, OrderType::Limit, Some(90.0), 1.0)
            .unwrap_err();
        match err {
            MarketplaceError::PriceBandViolation { price, .. } => {
                assert!((price - 90.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn engine_accepts_limit_order_on_band_boundary() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);
        // Exactly at the lower and upper boundaries should be accepted.
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(95.0), 1.0)
            .is_ok());
        assert!(e
            .submit_order(OrderSide::Sell, OrderType::Limit, Some(105.0), 1.0)
            .is_ok());
    }

    #[test]
    fn engine_accepts_market_order_regardless_of_band() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);
        // Market orders have no explicit price — they are always accepted.
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Market, None, 1.0)
            .is_ok());
    }

    #[test]
    fn no_band_accepts_any_price() {
        let mut e = MatchingEngine::new(); // no band
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(999.0), 1.0)
            .is_ok());
    }

    // ── Momentum ("on a roll") — upside expands to 20 % ───────────────────

    #[test]
    fn not_on_a_roll_without_enough_trades() {
        let mut band = PriceBand::new(100.0, 5.0); // streak_threshold = 3
        band.record_trade(to_price(101.0));
        band.record_trade(to_price(102.0));
        // Only 2 trades — need 3 for streak.
        assert!(!band.is_on_a_roll());
        assert!((band.upper_limit() - 105.0).abs() < 1e-4);
    }

    #[test]
    fn on_a_roll_after_streak_of_up_ticks() {
        let mut band = PriceBand::new(100.0, 5.0);
        band.record_trade(to_price(101.0));
        band.record_trade(to_price(102.0));
        band.record_trade(to_price(103.0));
        assert!(band.is_on_a_roll());
        // Upside cap expands to 20 %.
        assert!((band.upper_limit() - 120.0).abs() < 1e-4);
    }

    #[test]
    fn streak_broken_by_flat_or_down_tick() {
        let mut band = PriceBand::new(100.0, 5.0);
        band.record_trade(to_price(101.0));
        band.record_trade(to_price(102.0));
        band.record_trade(to_price(101.5)); // down-tick breaks streak
        assert!(!band.is_on_a_roll());
        assert!((band.upper_limit() - 105.0).abs() < 1e-4);
    }

    #[test]
    fn engine_accepts_order_up_to_20pct_when_on_a_roll() {
        let mut e = MatchingEngine::new();
        // Use streak_threshold = 3 via PriceBand::with_config for clarity.
        let band = PriceBand::with_config(100.0, 5.0, 5.0, 20.0, 3);
        e.set_price_band_custom(band);

        // Build up a 3-trade up-tick streak by placing matching limit pairs.
        let prices = [101.0_f64, 102.0, 103.0];
        for p in prices {
            e.submit_order(OrderSide::Sell, OrderType::Limit, Some(p), 1.0)
                .unwrap();
            e.submit_order(OrderSide::Buy, OrderType::Limit, Some(p), 1.0)
                .unwrap();
        }

        assert!(e.price_band().unwrap().is_on_a_roll());

        // An order at 118.0 (within 20 % band) must now be accepted.
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(118.0), 1.0)
            .is_ok());

        // An order above 120.0 must still be rejected.
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(121.0), 1.0)
            .is_err());
    }

    #[test]
    fn engine_still_rejects_above_20pct_even_on_a_roll() {
        let mut e = MatchingEngine::new();
        let band = PriceBand::with_config(100.0, 5.0, 5.0, 20.0, 3);
        e.set_price_band_custom(band);

        // Seed the streak manually via PriceBand::record_trade through the engine.
        // We do this by executing 3 ascending trades.
        for p in [101.0_f64, 102.0, 103.0] {
            e.submit_order(OrderSide::Sell, OrderType::Limit, Some(p), 1.0)
                .unwrap();
            e.submit_order(OrderSide::Buy, OrderType::Limit, Some(p), 1.0)
                .unwrap();
        }

        assert!(e.price_band().unwrap().is_on_a_roll());
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(120.01), 1.0)
            .is_err());
    }

    // ── Daily reference reset ──────────────────────────────────────────────

    #[test]
    fn reset_daily_reference_clears_streak_and_moves_band() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);

        // Seed a streak.
        for p in [101.0_f64, 102.0, 103.0] {
            e.submit_order(OrderSide::Sell, OrderType::Limit, Some(p), 1.0)
                .unwrap();
            e.submit_order(OrderSide::Buy, OrderType::Limit, Some(p), 1.0)
                .unwrap();
        }
        assert!(e.price_band().unwrap().is_on_a_roll());

        // New day — reset to 110.0 reference.
        e.reset_daily_reference(110.0);

        let band = e.price_band().unwrap();
        assert!(!band.is_on_a_roll()); // streak cleared
        assert!((band.lower_limit() - 104.5).abs() < 1e-2);
        assert!((band.upper_limit() - 115.5).abs() < 1e-2);
    }

    #[test]
    fn clear_price_band_removes_restriction() {
        let mut e = MatchingEngine::new();
        e.set_price_band(100.0, 5.0);
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(200.0), 1.0)
            .is_err());
        e.clear_price_band();
        assert!(e
            .submit_order(OrderSide::Buy, OrderType::Limit, Some(200.0), 1.0)
            .is_ok());
    }

    #[test]
    fn marketplace_error_display() {
        let err = MarketplaceError::PriceBandViolation {
            price: 110.0,
            lower_limit: 95.0,
            upper_limit: 105.0,
        };
        let msg = err.to_string();
        assert!(msg.contains("110.0"));
        assert!(msg.contains("95.0"));
        assert!(msg.contains("105.0"));
    }
}
