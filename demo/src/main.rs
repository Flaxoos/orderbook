//! # Order Book Demo
//!
//! Demonstrates various features and behaviors of the order book implementation.
//!
//! This demo shows:
//! - Basic order matching
//! - Partial fills
//! - Price-time priority
//! - Complex market scenarios

use order_book_core::types::{Asset, Instrument};
use order_book_core::{
    format_price, format_quantity, price_to_minor_units, quantity_to_minor_units, OrderBook, Side,
    Trade,
};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Main entry point that runs all demo scenarios.
fn main() {
    println!("=== Limit Order Book Demo ===\n");

    let btc = Asset::new("BTC", 6); // Base: BTC (6 decimals)
    let usdt = Asset::new("USDT", 2); // Quote: USDT (2 decimals)
    let instrument = Instrument::new(btc, usdt);

    println!("Instrument details: {}", instrument);
    let mut book1 = OrderBook::new(instrument.clone());
    demo_basic_matching(&mut book1);

    let mut book2 = OrderBook::new(instrument.clone());
    demo_partial_fills(&mut book2);

    let mut book3 = OrderBook::new(instrument.clone());
    demo_price_time_priority(&mut book3);

    let mut book4 = OrderBook::new(instrument.clone());
    demo_complex_scenario(&mut book4);
}

/// Demonstrates basic order matching between buy and sell orders.
///
/// Shows how a buy order at a specific price matches exactly with
/// a sell order at the same price.
fn demo_basic_matching(book: &mut OrderBook) {
    println!("-----------------------");
    println!("1. Basic Matching Demo:");
    println!("-----------------------");

    let trades = place_order_decimal(book, Side::Buy, "100.00", "0.010", 1)
        .expect("Failed to place BUY order");
    print_trades(&trades, book);
    print_book_state(book);

    let trades = place_order_decimal(book, Side::Sell, "100.00", "0.010", 2)
        .expect("Failed to place SELL order");
    print_trades(&trades, book);
    print_book_state(book);
}

/// Demonstrates partial order fills.
///
/// Shows what happens when orders are only partially matched,
/// leaving remaining quantity in the book.
fn demo_partial_fills(book: &mut OrderBook) {
    println!("---------------------");
    println!("2. Partial Fill Demo:");
    println!("---------------------");

    place_order_decimal(book, Side::Buy, "100.00", "0.015", 1).expect("Failed to place BUY order");

    let trades = place_order_decimal(book, Side::Sell, "100.00", "0.010", 2)
        .expect("Failed to place SELL order");
    print_trades(&trades, book);
    print_book_state(book);

    let trades = place_order_decimal(book, Side::Sell, "100.00", "0.010", 3)
        .expect("Failed to place SELL order");
    print_trades(&trades, book);
    print_book_state(book);
}

/// Demonstrates price-time priority matching rules.
///
/// Shows how orders are matched first by best price, then by
/// arrival time (FIFO) for orders at the same price level.
fn demo_price_time_priority(book: &mut OrderBook) {
    println!("----------------------------");
    println!("3. Price-Time Priority Demo:");
    println!("----------------------------");

    place_order_decimal(book, Side::Buy, "99.00", "0.010", 1).unwrap();
    place_order_decimal(book, Side::Buy, "100.00", "0.010", 2).unwrap();
    place_order_decimal(book, Side::Buy, "100.00", "0.010", 3).unwrap();

    print_book_state(book);

    let trades = place_order_decimal(book, Side::Sell, "99.00", "0.025", 4).unwrap();

    print_trades(&trades, book);

    print_book_state(book);
}

/// Demonstrates a complex market scenario with multiple price levels.
///
/// Shows aggressive orders that cross the spread and match against
/// multiple price levels, illustrating realistic market behavior.
fn demo_complex_scenario(book: &mut OrderBook) {
    println!("---------------------------");
    println!("4. Complex Market Scenario:");
    println!("---------------------------");

    println!("Building initial order book:");
    place_order_decimal(book, Side::Buy, "98.00", "0.020", 1).unwrap();
    place_order_decimal(book, Side::Buy, "99.00", "0.015", 2).unwrap();
    place_order_decimal(book, Side::Buy, "100.00", "0.010", 3).unwrap();
    place_order_decimal(book, Side::Sell, "101.00", "0.010", 4).unwrap();
    place_order_decimal(book, Side::Sell, "102.00", "0.015", 5).unwrap();
    place_order_decimal(book, Side::Sell, "103.00", "0.020", 6).unwrap();

    print_book_state(book);

    println!("\nLarge aggressive BUY order crosses spread:");
    let trades = place_order_decimal(book, Side::Buy, "102.00", "0.030", 7).unwrap();

    print_trades(&trades, book);

    print_book_state(book);

    println!("\nLarge aggressive SELL order:");
    let trades = place_order_decimal(book, Side::Sell, "98.00", "0.040", 8).unwrap();

    print_trades(&trades, book);

    print_book_state(book);
}

/// Prints a list of executed trades in a formatted way.
///
/// # Arguments
///
/// * `trades` - Slice of trades to display
/// * `book` - Reference to the order book for asset information
fn print_trades(trades: &[Trade], book: &OrderBook) {
    if trades.is_empty() {
        println!("--No trades executed");
    } else {
        println!("--Trades executed:");
        for trade in trades {
            let price_str = format_price(trade.price, &book.instrument.quote);
            let qty_str = format_quantity(trade.quantity, &book.instrument.base);
            println!(
                "----Trade: {} @ {} (maker: {}, taker: {})",
                qty_str, price_str, trade.maker_id, trade.taker_id
            );
        }
    }
}

/// Prints the current state of the order book showing best bid and ask.
///
/// # Arguments
///
/// * `book` - Reference to the order book to display
fn print_book_state(book: &OrderBook) {
    println!("--Book state:");
    match book.best_buy() {
        Some((price, qty)) => {
            let price_str = format_price(price, &book.instrument.quote);
            let qty_str = format_quantity(qty, &book.instrument.base);
            println!("----Best BUY:  {} @ {}", qty_str, price_str);
        }
        None => println!("----Best BUY:  None"),
    }
    match book.best_sell() {
        Some((price, qty)) => {
            let price_str = format_price(price, &book.instrument.quote);
            let qty_str = format_quantity(qty, &book.instrument.base);
            println!("----Best SELL: {} @ {}", qty_str, price_str);
        }
        None => println!("----Best SELL: None"),
    }
    println!();
}

/// Helper to convert decimal values to minor units for orders
fn place_order_decimal(
    book: &mut OrderBook,
    side: Side,
    price_decimal: &str,
    quantity_decimal: &str,
    id: u64,
) -> Result<Vec<Trade>, order_book_core::OrderBookError> {
    println!(
        "--Placing {} order: ID={}, Price={}, Qty={}",
        side, id, price_decimal, quantity_decimal
    );
    let price = Decimal::from_str(price_decimal).unwrap();
    let quantity = Decimal::from_str(quantity_decimal).unwrap();

    let price_minor = price_to_minor_units(price, &book.instrument.quote).unwrap();
    let quantity_minor = quantity_to_minor_units(quantity, &book.instrument.base).unwrap();

    book.place_order(side, price_minor, quantity_minor, id)
}
