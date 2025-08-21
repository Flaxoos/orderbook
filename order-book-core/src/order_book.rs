use crate::types::{
    Id, Instrument, Order, OrderBookError, Price, PriceAndQuantity, PriceLevel, Quantity, Side,
    Timestamp, Trade, Trades,
};
use std::collections::{BTreeMap, HashSet};

/// Result of matching against a price level, indicating what cache updates are needed.
#[derive(Debug, PartialEq)]
enum LevelMatchResult {
    /// Level still has orders, was not the best level
    Matched,
    /// Level still has orders, was the best level (cache update needed)
    MatchedBestLevel,
    /// Level is now empty and should be removed
    EmptyLevel,
    /// Level is now empty, was the best level (remove + cache update needed)
    EmptyBestLevel,
}

/// A limit order book that maintains buy and sell orders.
///
/// Orders are organized by price level, with price-time priority for matching.
/// Buy orders (bids) are sorted in descending price order, sell orders (asks)
/// in ascending price order.
pub struct OrderBook {
    /// Instrument being traded
    pub instrument: Instrument,
    /// Buy orders (bids) organized by price level
    buy_side: BTreeMap<Price, PriceLevel>,
    /// Sell orders (asks) organized by price level
    sell_side: BTreeMap<Price, PriceLevel>,
    /// Counter for generating order timestamps
    next_timestamp: Timestamp,
    /// Set of order IDs currently resting in the book
    id_index: HashSet<Id>,
    /// Cached best buy price and quantity
    best_buy: Option<PriceAndQuantity>,
    /// Cached best sell price and quantity
    best_sell: Option<PriceAndQuantity>,
}

impl OrderBook {
    /// Creates a new empty order book for the specified instrument and a default
    /// alignment policy of `AlignmentPolicy::Reject`.
    pub fn new(instrument: Instrument) -> Self {
        OrderBook {
            instrument,
            buy_side: BTreeMap::new(),
            sell_side: BTreeMap::new(),
            next_timestamp: 0,
            id_index: HashSet::new(),
            best_buy: None,
            best_sell: None,
        }
    }

    /// Places an order in the book and returns any resulting trades.
    ///
    /// The order will first attempt to match against existing orders on the
    /// opposite side. Any remaining quantity will be added to the book.
    ///
    /// # Arguments
    ///
    /// * `side` - Whether this is a buy or sell order
    /// * `price` - Price per unit
    /// * `quantity` - Number of units to trade
    /// * `id` - Unique identifier for the order
    ///
    /// # Returns
    ///
    /// A vector of trades that occurred as a result of this order
    pub fn place_order(
        &mut self,
        side: Side,
        price: Price,
        quantity: Quantity,
        id: Id,
    ) -> Result<Trades, OrderBookError> {
        if self.id_index.contains(&id) {
            return Err(OrderBookError::DuplicateOrderId(id));
        }
        if quantity == 0 {
            return Err(OrderBookError::ZeroQuantity { id, quantity });
        }

        let timestamp = self.next_timestamp;
        self.next_timestamp += 1;

        let mut incoming_order = Order::new(id, side, price, quantity, timestamp);

        let trades = self.match_incoming_order(&mut incoming_order);

        if incoming_order.quantity > 0 {
            self.add_order_to_book(incoming_order);
            self.id_index.insert(id);
        }

        Ok(trades)
    }

    /// Returns the best (highest) buy price and total quantity at that level.
    ///
    /// # Returns
    ///
    /// `Some(PriceAndQuantity)` if buy orders exist, `None` otherwise
    pub fn best_buy(&self) -> Option<PriceAndQuantity> {
        self.best_buy
    }

    /// Returns the best (lowest) sell price and total quantity at that level.
    ///
    /// # Returns
    ///
    /// `Some(PriceAndQuantity)` if sell orders exist, `None` otherwise
    pub fn best_sell(&self) -> Option<PriceAndQuantity> {
        self.best_sell
    }

    /// Returns market depth information for the specified side.
    ///
    /// For buy side, returns prices in descending order (best first).
    /// For sell side, returns prices in ascending order (best first).
    ///
    /// # Arguments
    ///
    /// * `side` - Which side of the book to query
    /// * `levels` - Maximum number of price levels to return
    ///
    /// # Returns
    ///
    /// Vector of (price, total_quantity) tuples
    #[allow(dead_code)]
    pub fn depth(&self, side: Side, levels: usize) -> Vec<PriceAndQuantity> {
        let book_side = match side {
            Side::Buy => &self.buy_side,
            Side::Sell => &self.sell_side,
        };

        let iter: Box<dyn Iterator<Item = (&Price, &PriceLevel)>> = match side {
            Side::Buy => Box::new(book_side.iter().rev()),
            Side::Sell => Box::new(book_side.iter()),
        };

        iter.take(levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect()
    }

    /// Returns true if the order book has no orders on either side.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buy_side.is_empty() && self.sell_side.is_empty()
    }

    /// Updates the cached best buy price and quantity.
    ///
    /// Recalculates the best buy from the buy_side BTreeMap and caches the result.
    /// This should be called whenever the buy side of the book is modified.
    fn set_best_buy(&mut self) {
        self.best_buy = self
            .buy_side
            .iter()
            .next_back()
            .map(|(price, level)| (*price, level.total_quantity));
    }

    /// Updates the cached best sell price and quantity.
    ///
    /// Recalculates the best sell from the sell_side BTreeMap and caches the result.
    /// This should be called whenever the sell side of the book is modified.
    fn update_cached_best_sell(&mut self) {
        self.best_sell = self
            .sell_side
            .iter()
            .next()
            .map(|(price, level)| (*price, level.total_quantity));
    }

    /// Attempts to match an incoming order against existing orders.
    ///
    /// For buy orders, matches against sell orders at or below the buy price.
    /// For sell orders, matches against buy orders at or above the sell price.
    /// Orders are matched in price-time priority.
    fn match_incoming_order(&mut self, incoming: &mut Order) -> Trades {
        let mut trades = Vec::new();

        match incoming.side {
            Side::Buy => {
                while incoming.quantity > 0 {
                    // Get the best matching price level
                    let best_price = match self.sell_side.range(..=incoming.price).next() {
                        Some((price, _)) => *price,
                        None => break, // No more matching levels
                    };
                    
                    // Process this single price level completely
                    let match_result = Self::match_price_level(
                        incoming,
                        &mut trades,
                        best_price,
                        &mut self.sell_side,
                        &mut self.id_index,
                    );

                    match match_result {
                        LevelMatchResult::EmptyBestLevel => {
                            self.sell_side.remove(&best_price);
                            self.update_cached_best_sell();
                        }
                        LevelMatchResult::EmptyLevel => {
                            self.sell_side.remove(&best_price);
                        }
                        LevelMatchResult::MatchedBestLevel => {
                            self.update_cached_best_sell();
                        }
                        LevelMatchResult::Matched => {
                            // No cache update needed
                        }
                    }
                }
            }
            Side::Sell => {
                while incoming.quantity > 0 {
                    // Get the best matching price level
                    let best_price = match self.buy_side.range(incoming.price..).next_back() {
                        Some((price, _)) => *price,
                        None => break, // No more matching levels
                    };
                    
                    // Process this single price level completely
                    let match_result = Self::match_price_level(
                        incoming,
                        &mut trades,
                        best_price,
                        &mut self.buy_side,
                        &mut self.id_index,
                    );

                    match match_result {
                        LevelMatchResult::EmptyBestLevel => {
                            self.buy_side.remove(&best_price);
                            self.set_best_buy();
                        }
                        LevelMatchResult::EmptyLevel => {
                            self.buy_side.remove(&best_price);
                        }
                        LevelMatchResult::MatchedBestLevel => {
                            self.set_best_buy();
                        }
                        // No cache update needed
                        LevelMatchResult::Matched => {}
                    }
                }
            }
        }

        trades
    }

    /// Helper method to match against a single price level on a specific book side.
    ///
    /// This eliminates the duplication between Buy and Sell matching logic by
    /// parameterizing the side-specific behaviors.
    ///
    /// Returns matching result to guide cache updates.
    fn match_price_level(
        incoming: &mut Order,
        trades: &mut Vec<Trade>,
        price: Price,
        book_side: &mut BTreeMap<Price, PriceLevel>,
        id_index: &mut HashSet<Id>,
    ) -> LevelMatchResult {
        // Check if this price level is the best before modifying it
        let level_was_best = match incoming.side {
            Side::Buy => book_side.iter().next().map(|(p, _)| *p) == Some(price),
            Side::Sell => book_side.iter().next_back().map(|(p, _)| *p) == Some(price),
        };

        // compute whether this level becomes empty *inside* a block
        let level_is_empty = if let Some(level) = book_side.get_mut(&price) {
            Self::match_against_level(incoming, level, trades, id_index);
            level.is_empty()
        } else {
            false
        };

        match (level_is_empty, level_was_best) {
            (true, true) => LevelMatchResult::EmptyBestLevel,
            (true, false) => LevelMatchResult::EmptyLevel,
            (false, true) => LevelMatchResult::MatchedBestLevel,
            (false, false) => LevelMatchResult::Matched,
        }
    }

    /// Matches an incoming order against a specific price level.
    ///
    /// Continues matching until either the incoming order is fully filled
    /// or the price level is exhausted.
    // Free/assoc fn; no &mut self here
    fn match_against_level(
        incoming: &mut Order,
        level: &mut PriceLevel,
        trades: &mut Vec<Trade>,
        id_index: &mut HashSet<Id>,
    ) {
        while incoming.quantity > 0 && !level.orders.is_empty() {
            let resting = level.orders.front().expect("front exists");
            let match_qty = incoming.quantity.min(resting.quantity);

            trades.push(Trade::new(level.price, match_qty, resting.id, incoming.id));
            incoming.quantity -= match_qty;

            if match_qty == resting.quantity {
                // fully consumed: pop & deindex
                let removed = level.remove_order().expect("front existed");
                id_index.remove(&removed.id);
            } else {
                // partial: shrink front
                level.update_front_order_quantity(resting.quantity - match_qty);
            }
        }
    }

    /// Adds an order to the appropriate side of the book.
    ///
    /// Creates a new price level if one doesn't exist at the order's price.
    fn add_order_to_book(&mut self, order: Order) {
        let book_side = match order.side {
            Side::Buy => &mut self.buy_side,
            Side::Sell => &mut self.sell_side,
        };

        book_side
            .entry(order.price)
            .or_insert_with(|| PriceLevel::new(order.price))
            .add_order(order.clone());

        // Update cache when adding orders that might affect best prices
        match order.side {
            Side::Buy => self.set_best_buy(),
            Side::Sell => self.update_cached_best_sell(),
        }
    }
}
#[cfg(test)]
mod order_book_tests {
    use super::*;
    use crate::test_support::*;
    use crate::types::OrderBookError;

    #[test]
    fn test_id_uniqueness() {
        let mut order_book = new_book();
        let result1 = order_book.place_order(Side::Buy, price("100.00"), quantity("0.010"), 1);
        assert!(result1.is_ok());
        let result2 = order_book.place_order(Side::Buy, price("100.00"), quantity("0.010"), 1);
        assert!(matches!(result2, Err(OrderBookError::DuplicateOrderId(1))));
    }

    #[test]
    fn test_zero_quantity_error() {
        let mut order_book = new_book();
        let result = order_book.place_order(Side::Buy, price("100.00"), 0, 1);
        assert!(matches!(
            result,
            Err(OrderBookError::ZeroQuantity { id: 1, quantity: 0 })
        ));
    }
    // --- core matching tests ---

    #[test]
    fn basic_full_fill_resting_ask_hit_by_buy() {
        let mut order_book = new_book();

        // Maker: SELL 0.010000 @ 100.00
        let a_price = price("100.00");
        let a_quantity = quantity("0.010000");
        order_book
            .place_order(Side::Sell, a_price, a_quantity, 1)
            .unwrap();

        // Taker: BUY same quantity at 100.00 (crosses)
        let trades = order_book
            .place_order(Side::Buy, a_price, a_quantity, 2)
            .unwrap();
        assert_eq!(trades.len(), 1);
        let t = &trades[0];
        assert_eq!(t.price, a_price);
        assert_eq!(t.quantity, a_quantity);
        assert_eq!(t.maker_id, 1);
        assert_eq!(t.taker_id, 2);

        // Book empty
        assert!(order_book.best_buy().is_none());
        assert!(order_book.best_sell().is_none());
    }

    #[test]
    fn partial_fill_and_remainder_resting_on_same_side() {
        let mut order_book = new_book();

        // Maker: SELL 0.005000 @ 100.00
        order_book
            .place_order(Side::Sell, price("100.00"), quantity("0.005000"), 1)
            .unwrap();

        // Taker: BUY 0.008000 @ 100.00 -> fills 0.005000, leaves 0.003000 as bid
        let trades = order_book
            .place_order(Side::Buy, price("100.00"), quantity("0.008000"), 2)
            .unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, quantity("0.005000"));

        // Best buy is remainder @ 100.00 for 0.003000
        let (bb_price, bb_quantity) = order_book.best_buy().expect("has bid");
        assert_eq!(bb_price, price("100.00"));
        assert_eq!(bb_quantity, quantity("0.003000"));

        // No asks
        assert!(order_book.best_sell().is_none());
    }

    #[test]
    fn price_time_priority_within_level_and_across_levels() {
        let mut order_book = new_book();

        // Resting asks:
        // Better price first: 99.99 (id=10 quantity=0.002)
        order_book
            .place_order(Side::Sell, price("99.99"), quantity("0.002"), 10)
            .unwrap();
        // Worse price: 100.00 (two FIFO orders id=11 then id=12)
        order_book
            .place_order(Side::Sell, price("100.00"), quantity("0.003"), 11)
            .unwrap();
        order_book
            .place_order(Side::Sell, price("100.00"), quantity("0.004"), 12)
            .unwrap();

        // Incoming BUY crosses for total 0.007:
        let trades = order_book
            .place_order(Side::Buy, price("150.00"), quantity("0.007"), 99)
            .unwrap();
        assert_eq!(trades.len(), 3);

        // 1) hit 99.99 (id=10) for 0.002
        assert_eq!(trades[0].price, price("99.99"));
        assert_eq!(trades[0].quantity, quantity("0.002"));
        assert_eq!(trades[0].maker_id, 10);

        // 2) then 100.00 id=11 for 0.003
        assert_eq!(trades[1].price, price("100.00"));
        assert_eq!(trades[1].quantity, quantity("0.003"));
        assert_eq!(trades[1].maker_id, 11);

        // 3) then 100.00 id=12 for 0.002
        assert_eq!(trades[2].price, price("100.00"));
        assert_eq!(trades[2].quantity, quantity("0.002"));
        assert_eq!(trades[2].maker_id, 12);

        // Book now has remaining ask 100.00 for 0.002
        let (ask_p, ask_q) = order_book.best_sell().expect("remaining ask");
        assert_eq!(ask_p, price("100.00"));
        assert_eq!(ask_q, quantity("0.002"));

        // No bids
        assert!(order_book.best_buy().is_none());
    }

    #[test]
    fn best_buy_and_best_sell_report_top_of_book() {
        let mut order_book = new_book();

        // Two bids at different prices
        order_book
            .place_order(Side::Buy, price("99.50"), quantity("0.010"), 1)
            .unwrap();
        order_book
            .place_order(Side::Buy, price("99.75"), quantity("0.020"), 2)
            .unwrap();

        // One ask
        order_book
            .place_order(Side::Sell, price("100.10"), quantity("0.015"), 3)
            .unwrap();

        // Best BUY is highest price (99.75)
        let (bb_p, bb_q) = order_book.best_buy().unwrap();
        assert_eq!(bb_p, price("99.75"));
        assert_eq!(bb_q, quantity("0.020"));

        // Best SELL is lowest price (100.10)
        let (ba_p, ba_q) = order_book.best_sell().unwrap();
        assert_eq!(ba_p, price("100.10"));
        assert_eq!(ba_q, quantity("0.015"));
    }

    #[test]
    fn test_cached_best_prices_update_during_matching() {
        let mut order_book = new_book();

        // Setup: Create multiple price levels on both sides
        // Sell side: 99.00 (qty=1), 99.50 (qty=2), 100.00 (qty=3)
        order_book.place_order(Side::Sell, price("99.00"), quantity("0.001"), 1).unwrap();
        order_book.place_order(Side::Sell, price("99.50"), quantity("0.002"), 2).unwrap();
        order_book.place_order(Side::Sell, price("100.00"), quantity("0.003"), 3).unwrap();
        
        // Buy side: 98.00 (qty=1), 98.50 (qty=2)
        order_book.place_order(Side::Buy, price("98.00"), quantity("0.001"), 4).unwrap();
        order_book.place_order(Side::Buy, price("98.50"), quantity("0.002"), 5).unwrap();

        // Verify initial cached best prices
        assert_eq!(order_book.best_sell().unwrap(), (price("99.00"), quantity("0.001")));
        assert_eq!(order_book.best_buy().unwrap(), (price("98.50"), quantity("0.002")));

        // Test 1: Incoming buy that removes best sell level and updates cache
        let trades = order_book.place_order(Side::Buy, price("99.25"), quantity("0.001"), 6).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, price("99.00")); // Matched at 99.00
        
        // Cache should be updated - best sell is now 99.50
        assert_eq!(order_book.best_sell().unwrap(), (price("99.50"), quantity("0.002")));
        assert_eq!(order_book.best_buy().unwrap(), (price("98.50"), quantity("0.002"))); // Unchanged

        // Test 2: Incoming buy that partially fills best sell level (cache updates quantity)
        let trades = order_book.place_order(Side::Buy, price("99.50"), quantity("0.001"), 7).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, quantity("0.001"));
        
        // Cache should be updated - best sell quantity reduced
        assert_eq!(order_book.best_sell().unwrap(), (price("99.50"), quantity("0.001")));

        // Test 3: Incoming sell that removes best buy level and updates cache
        let trades = order_book.place_order(Side::Sell, price("98.25"), quantity("0.002"), 8).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, price("98.50")); // Matched at 98.50
        
        // Cache should be updated - best buy is now 98.00
        assert_eq!(order_book.best_buy().unwrap(), (price("98.00"), quantity("0.001")));

        // Test 4: Large order that sweeps multiple levels and updates cache correctly
        let trades = order_book.place_order(Side::Buy, price("101.00"), quantity("0.010"), 9).unwrap();
        assert_eq!(trades.len(), 2); // Should match 99.50 (0.001) and 100.00 (0.003)
        
        // After sweeping, sell side should be empty
        assert!(order_book.best_sell().is_none());
        
        // Remainder should be added as new best buy
        assert_eq!(order_book.best_buy().unwrap(), (price("101.00"), quantity("0.006"))); // 10 - 1 - 3 = 6
    }

    // --- sanity: PriceLevel FIFO using actual Order ---

    #[test]
    fn price_level_fifo_with_orders() {
        let mut lvl = PriceLevel::new(price("100.00"));

        let o1 = Order::new(1, Side::Buy, price("100.00"), quantity("0.003"), 10);
        let o2 = Order::new(2, Side::Buy, price("100.00"), quantity("0.002"), 11);
        lvl.add_order(o1.clone());
        lvl.add_order(o2.clone());

        // FIFO preserved
        assert_eq!(lvl.orders.front().unwrap().id, 1);
        assert_eq!(lvl.orders.back().unwrap().id, 2);
        assert_eq!(lvl.total_quantity, quantity("0.005"));

        // Partial consume front
        lvl.update_front_order_quantity(quantity("0.001"));
        assert_eq!(lvl.orders.front().unwrap().quantity, quantity("0.001"));
        assert_eq!(lvl.total_quantity, quantity("0.003")); // 0.001 + 0.002

        // Remove front (o1)
        let removed = lvl.remove_order().unwrap();
        assert_eq!(removed.id, 1);
        assert_eq!(lvl.total_quantity, quantity("0.002"));

        // Remove last (o2)
        let removed2 = lvl.remove_order().unwrap();
        assert_eq!(removed2.id, 2);
        assert_eq!(lvl.total_quantity, 0);
        assert!(lvl.is_empty());
    }
}
