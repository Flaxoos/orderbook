#![cfg(test)]

use crate::types::{Asset, Instrument, Price, Quantity};
use crate::OrderBook;
use rust_decimal::Decimal;
use std::str::FromStr;

pub(crate) fn std_instrument() -> Instrument {
    // Quote: USDT (2 dp) -> tick step 1 minor unit = 0.01
    let usdt = Asset::new("USDT", 2);
    // Base: BTC (6 dp) -> lot step 1_000 minor units = 0.001
    let btc = Asset::new("BTC", 6);
    Instrument::new(btc, usdt)
}

pub(crate) fn new_book() -> OrderBook {
    OrderBook::new(std_instrument())
}

// Align helpers (Decimal → u128 minor units)
pub(crate) fn price(p: &str) -> Price {
    let d = Decimal::from_str(p).unwrap();
    let q_decimals = std_instrument().quote.decimals;
    crate::units::to_minor_units(d, q_decimals).unwrap()
}
pub(crate) fn quantity(q: &str) -> Quantity {
    let d = Decimal::from_str(q).unwrap();
    let b_decimals = std_instrument().base.decimals;
    crate::units::to_minor_units(d, b_decimals).unwrap()
}
