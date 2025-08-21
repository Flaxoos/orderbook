use crate::types::{Id, Instrument, Order, OrderBookError, Price, PriceAndQuantity, PriceLevel, Quantity, Side, Timestamp, Trade, Trades};
use std::collections::{BTreeMap, HashSet};

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
        self.buy_side
            .iter()
            .next_back()
            .map(|(price, level)| (*price, level.total_quantity))
    }

    /// Returns the best (lowest) sell price and total quantity at that level.
    ///
    /// # Returns
    ///
    /// `Some(PriceAndQuantity)` if sell orders exist, `None` otherwise
    pub fn best_sell(&self) -> Option<PriceAndQuantity> {
        self.sell_side
            .iter()
            .next()
            .map(|(price, level)| (*price, level.total_quantity))
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

    /// Attempts to match an incoming order against existing orders.
    ///
    /// For buy orders, matches against sell orders at or below the buy price.
    /// For sell orders, matches against buy orders at or above the sell price.
    /// Orders are matched in price-time priority.
    fn match_incoming_order(&mut self, incoming: &mut Order) -> Trades {
        let mut trades = Vec::new();

        match incoming.side {
            Side::Buy => {
                let prices_to_match: Vec<Price> = self
                    .sell_side
                    .range(..=incoming.price)
                    .map(|(price, _)| *price)
                    .collect();

                for price in prices_to_match {
                    if incoming.quantity == 0 {
                        break;
                    }

                    // compute whether this level becomes empty *inside* a block
                    let remove_level = if let Some(level) = self.sell_side.get_mut(&price) {
                        Self::match_against_level(incoming, level, &mut trades, &mut self.id_index);
                        level.is_empty()
                    } else {
                        false
                    };

                    if remove_level {
                        self.sell_side.remove(&price);
                    }
                }
            }
            Side::Sell => {
                let prices_to_match: Vec<Price> = self
                    .buy_side
                    .range(incoming.price..)
                    .rev()
                    .map(|(price, _)| *price)
                    .collect();

                for price in prices_to_match {
                    if incoming.quantity == 0 {
                        break;
                    }

                    let remove_level = if let Some(level) = self.buy_side.get_mut(&price) {
                        Self::match_against_level(incoming, level, &mut trades, &mut self.id_index);
                        level.is_empty()
                    } else {
                        false
                    };

                    if remove_level {
                        self.buy_side.remove(&price);
                    }
                }
            }
        }

        trades
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
            .add_order(order);
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
        assert!(matches!(result, Err(OrderBookError::ZeroQuantity { id: 1, quantity: 0 })));
    }
    // --- core matching tests ---

    #[test]
    fn basic_full_fill_resting_ask_hit_by_buy() {
        let mut order_book = new_book();

        // Maker: SELL 0.010000 @ 100.00
        let a_price = price("100.00");
        let a_quantity = quantity("0.010000");
        order_book.place_order(Side::Sell, a_price, a_quantity, 1).unwrap();

        // Taker: BUY same quantity at 100.00 (crosses)
        let trades = order_book.place_order(Side::Buy, a_price, a_quantity, 2).unwrap();
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
        order_book.place_order(Side::Sell, price("100.00"), quantity("0.005000"), 1).unwrap();

        // Taker: BUY 0.008000 @ 100.00 -> fills 0.005000, leaves 0.003000 as bid
        let trades = order_book.place_order(Side::Buy, price("100.00"), quantity("0.008000"), 2).unwrap();
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
        order_book.place_order(Side::Sell, price("99.99"), quantity("0.002"), 10).unwrap();
        // Worse price: 100.00 (two FIFO orders id=11 then id=12)
        order_book.place_order(Side::Sell, price("100.00"), quantity("0.003"), 11).unwrap();
        order_book.place_order(Side::Sell, price("100.00"), quantity("0.004"), 12).unwrap();

        // Incoming BUY crosses for total 0.007:
        let trades = order_book.place_order(Side::Buy, price("150.00"), quantity("0.007"), 99).unwrap();
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
        order_book.place_order(Side::Buy, price("99.50"), quantity("0.010"), 1).unwrap();
        order_book.place_order(Side::Buy, price("99.75"), quantity("0.020"), 2).unwrap();

        // One ask
        order_book.place_order(Side::Sell, price("100.10"), quantity("0.015"), 3).unwrap();

        // Best BUY is highest price (99.75)
        let (bb_p, bb_q) = order_book.best_buy().unwrap();
        assert_eq!(bb_p, price("99.75"));
        assert_eq!(bb_q, quantity("0.020"));

        // Best SELL is lowest price (100.10)
        let (ba_p, ba_q) = order_book.best_sell().unwrap();
        assert_eq!(ba_p, price("100.10"));
        assert_eq!(ba_q, quantity("0.015"));
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
