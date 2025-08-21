//! # Order Book Core
//!
//! A high-performance limit order book implementation in Rust.
//!
//! This crate provides the core data structures and algorithms for maintaining
//! a limit order book with price-time priority matching. It supports placing
//! orders, automatic matching, and querying market depth.
//!
//! ## Example
//!
//! ```rust
//! use order_book_core::{OrderBook, Side};
//! use order_book_core::types::{Asset, Instrument};
//! 
//! // Create a BTC/USDT instrument
//! let usdt = Asset::new("USDT", 2);
//! let btc = Asset::new("BTC", 6);
//! let instrument = Instrument::new(btc, usdt);
//! let mut book = OrderBook::new(instrument);
//!
//! // Place a buy order (prices and quantities in minor units)
//! let trades = book.place_order(Side::Buy, 10000, 10000, 1).unwrap();
//! assert!(trades.is_empty()); // No matching orders yet
//!
//! // Place a matching sell order
//! let trades = book.place_order(Side::Sell, 10000, 5000, 2).unwrap();
//! assert_eq!(trades.len(), 1); // One trade executed
//! ```

mod units;
pub mod order_book;
#[cfg(test)]
pub(crate) mod test_support;
pub mod types;
pub use order_book::OrderBook;
pub use types::{Order, OrderBookError, Side, Trade, Trades};
pub use units::{
    format_price, format_quantity, price_from_minor_units, price_to_minor_units,
    quantity_from_minor_units, quantity_to_minor_units,
};

#[cfg(test)]
mod tests {
    use crate::Side;
    use crate::test_support::new_book;
    #[test]
    fn test_market_spread() {
        let mut book = new_book();

        // Using minor units: price*100 (2 decimals), qty*1000000 (6 decimals), but qty must be multiple of 1000
        book.place_order(Side::Buy, 9500, 100000, 1).unwrap();
        book.place_order(Side::Buy, 9400, 50000, 2).unwrap();
        book.place_order(Side::Sell, 10500, 100000, 3).unwrap();
        book.place_order(Side::Sell, 10600, 50000, 4).unwrap();

        assert_eq!(book.best_buy(), Some((9500, 100000)));
        assert_eq!(book.best_sell(), Some((10500, 100000)));

        let spread = book.best_sell().unwrap().0 - book.best_buy().unwrap().0;
        assert_eq!(spread, 1000);
    }

    #[test]
    fn test_aggressive_order_sweeps_multiple_levels() {
        let mut book = new_book();

        // Using minor units: price*100, qty*1000000 (must be multiple of 1000)
        book.place_order(Side::Sell, 10000, 10000, 1).unwrap();
        book.place_order(Side::Sell, 10100, 20000, 2).unwrap();
        book.place_order(Side::Sell, 10200, 30000, 3).unwrap();

        let trades = book.place_order(Side::Buy, 10500, 50000, 4).unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, 10000);
        assert_eq!(trades[0].quantity, 10000);
        assert_eq!(trades[1].price, 10100);
        assert_eq!(trades[1].quantity, 20000);
        assert_eq!(trades[2].price, 10200);
        assert_eq!(trades[2].quantity, 20000);

        assert_eq!(book.best_sell(), Some((10200, 10000)));
    }

    #[test]
    fn test_no_match_when_prices_dont_cross() {
        let mut book = new_book();

        book.place_order(Side::Buy, 9000, 100000, 1).unwrap();
        let trades = book.place_order(Side::Sell, 10000, 50000, 2).unwrap();

        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((9000, 100000)));
        assert_eq!(book.best_sell(), Some((10000, 50000)));
    }

    #[test]
    fn test_exact_price_match() {
        let mut book = new_book();

        book.place_order(Side::Buy, 10000, 50000, 1).unwrap();
        let trades = book.place_order(Side::Sell, 10000, 50000, 2).unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 10000);
        assert_eq!(trades[0].quantity, 50000);
        assert_eq!(book.best_buy(), None);
        assert_eq!(book.best_sell(), None);
    }

    #[test]
    fn test_multiple_partial_fills() {
        let mut book = new_book();

        book.place_order(Side::Buy, 10000, 25000, 1).unwrap();
        book.place_order(Side::Buy, 10000, 25000, 2).unwrap();
        book.place_order(Side::Buy, 10000, 25000, 3).unwrap();

        let trades = book.place_order(Side::Sell, 10000, 60000, 4).unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].quantity, 25000);
        assert_eq!(trades[1].quantity, 25000);
        assert_eq!(trades[2].quantity, 10000);

        assert_eq!(book.best_buy(), Some((10000, 15000)));
    }

    #[test]
    fn test_price_improvement() {
        let mut book = new_book();

        book.place_order(Side::Sell, 10000, 50000, 1).unwrap();

        let trades = book.place_order(Side::Buy, 10500, 50000, 2).unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 10000);
        assert_eq!(trades[0].quantity, 50000);
    }

    #[test]
    fn test_large_order_book_performance() {
        let mut book = new_book();

        for i in 1..=1000 {
            // Convert to minor units: price * 100, qty must be multiple of 1000 (lot size)
            book.place_order(Side::Buy, (1000 - i) * 100, 10000, i as u64).unwrap();
            book.place_order(Side::Sell, (1000 + i) * 100, 10000, (1000 + i) as u64).unwrap();
        }

        assert_eq!(book.best_buy(), Some((99900, 10000)));
        assert_eq!(book.best_sell(), Some((100100, 10000)));

        let trades = book.place_order(Side::Sell, 50000, 5000000, 2001).unwrap();
        assert_eq!(trades.len(), 500);

        let total_quantity: u128 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total_quantity, 5000000);
    }

    #[test]
    fn test_large_order_book() {
        let mut book = new_book();

        for i in 1..=100 {
            // Convert to minor units
            book.place_order(Side::Buy, (100 - i) * 100, 10000, i as u64).unwrap();
            book.place_order(Side::Sell, (100 + i) * 100, 10000, (100 + i) as u64).unwrap();
        }

        assert_eq!(book.best_buy(), Some((9900, 10000)));
        assert_eq!(book.best_sell(), Some((10100, 10000)));

        let trades = book.place_order(Side::Sell, 5000, 100000, 201).unwrap();
        assert_eq!(trades.len(), 10);

        for (i, trade) in trades.iter().enumerate() {
            assert_eq!(trade.price, (99 - i as u128) * 100);
            assert_eq!(trade.quantity, 10000);
        }
    }

    #[test]
    fn test_single_sided_book() {
        let mut book = new_book();

        book.place_order(Side::Buy, 10000, 10000, 1).unwrap();
        book.place_order(Side::Buy, 9900, 20000, 2).unwrap();
        book.place_order(Side::Buy, 9800, 30000, 3).unwrap();

        assert_eq!(book.best_buy(), Some((10000, 10000)));
        assert_eq!(book.best_sell(), None);

        let trades = book.place_order(Side::Buy, 10100, 50000, 4).unwrap();
        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((10100, 50000)));
    }

    #[test]
    fn test_maker_taker_id_correctness() {
        let mut book = new_book();

        book.place_order(Side::Buy, 10000, 10000, 123).unwrap();
        let trades = book.place_order(Side::Sell, 10000, 10000, 456).unwrap();

        assert_eq!(trades[0].maker_id, 123);
        assert_eq!(trades[0].taker_id, 456);
    }

    #[test]
    fn test_trade_price_is_resting_order_price() {
        let mut book = new_book();

        book.place_order(Side::Buy, 10000, 10000, 1).unwrap();
        let trades = book.place_order(Side::Sell, 9500, 10000, 2).unwrap();
        assert_eq!(trades[0].price, 10000);

        book.place_order(Side::Sell, 10500, 10000, 3).unwrap();
        let trades = book.place_order(Side::Buy, 11000, 10000, 4).unwrap();
        assert_eq!(trades[0].price, 10500);
    }
}
