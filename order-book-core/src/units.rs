use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use crate::types::{Asset, Price, Quantity};

#[inline]
fn pow10(n: u32) -> Decimal {
    // safe up to 10^28 for rust_decimal
    Decimal::from_i128_with_scale(1, 0) * Decimal::from_i128_with_scale(10_i128.pow(n), 0)
}

#[inline]
pub(crate) fn to_minor_units(val: Decimal, decimals: u8) -> Option<u128> {
    let m = pow10(decimals as u32);
    (val * m).trunc().to_u128()
}

#[inline]
pub(crate) fn from_minor_units(units: u128, decimals: u8) -> Decimal {
    let m = pow10(decimals as u32);
    Decimal::from_u128(units).unwrap() / m
}

/// Converts a decimal price to minor units for the given quote asset
pub fn price_to_minor_units(price: Decimal, quote_asset: &Asset) -> Option<Price> {
    to_minor_units(price, quote_asset.decimals)
}

/// Converts a decimal quantity to minor units for the given base asset
pub fn quantity_to_minor_units(quantity: Decimal, base_asset: &Asset) -> Option<Quantity> {
    to_minor_units(quantity, base_asset.decimals)
}

/// Converts minor units price back to decimal for the given quote asset
pub fn price_from_minor_units(price: Price, quote_asset: &Asset) -> Decimal {
    from_minor_units(price, quote_asset.decimals)
}

/// Converts minor units quantity back to decimal for the given base asset
pub fn quantity_from_minor_units(quantity: Quantity, base_asset: &Asset) -> Decimal {
    from_minor_units(quantity, base_asset.decimals)
}

/// Formats a price in minor units for display with the quote asset symbol
pub fn format_price(price: Price, quote_asset: &Asset) -> String {
    let decimal_price = price_from_minor_units(price, quote_asset);
    format!("{} {}", decimal_price, quote_asset.symbol)
}

/// Formats a quantity in minor units for display with the base asset symbol  
pub fn format_quantity(quantity: Quantity, base_asset: &Asset) -> String {
    let decimal_quantity = quantity_from_minor_units(quantity, base_asset);
    format!("{} {}", decimal_quantity, base_asset.symbol)
}