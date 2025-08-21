use derive_more::Display;
use std::borrow::Cow;
use std::collections::VecDeque;
use validator::Validate;

pub type Price = u128;
pub type Quantity = u128;

pub type PriceAndQuantity = (Price, Quantity);
pub type Id = u64;
pub type Timestamp = u64;

/// Represents a price level in the order book.
///
/// A price level contains all orders at the same price, maintaining
/// first-in-first-out (FIFO) ordering for time priority.
#[derive(Debug)]
pub(crate) struct PriceLevel {
    /// The price for this level
    pub(crate) price: Price,
    /// Queue of orders at this price level (FIFO ordering)
    pub(crate) orders: VecDeque<Order>,
    /// Total quantity available at this price level
    pub(crate) total_quantity: Quantity,
}

impl PriceLevel {
    /// Creates a new empty price level at the specified price.
    pub(crate) fn new(price: Price) -> Self {
        PriceLevel {
            price,
            orders: VecDeque::new(),
            total_quantity: 0,
        }
    }

    /// Adds an order to the back of the queue at this price level.
    pub(crate) fn add_order(&mut self, order: Order) {
        self.total_quantity += order.quantity;
        self.orders.push_back(order);
    }

    /// Removes and returns the order at the front of the queue.
    /// Returns None if the level is empty.
    pub(crate) fn remove_order(&mut self) -> Option<Order> {
        if let Some(order) = self.orders.pop_front() {
            self.total_quantity -= order.quantity;
            Some(order)
        } else {
            None
        }
    }

    /// Updates the quantity of the order at the front of the queue.
    /// Used when an order is partially filled.
    pub(crate) fn update_front_order_quantity(&mut self, new_quantity: Quantity) {
        if let Some(order) = self.orders.front_mut() {
            let old_quantity = order.quantity;
            order.quantity = new_quantity;
            self.total_quantity = self.total_quantity - old_quantity + new_quantity;
        }
    }

    /// Returns true if this price level has no orders.
    pub(crate) fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

#[derive(Display, Debug, Clone, PartialEq, Eq, Hash)]
#[display("{}", symbol)]
pub struct Asset {
    /// Symbol string
    pub symbol: Cow<'static, str>,
    /// Minor units for display/serde (e.g., USD=2, BTC=8)
    pub decimals: u8,
}

impl Asset {
    pub const fn new(symbol: &'static str, decimals: u8) -> Self {
        Self {
            symbol: Cow::Borrowed(symbol),
            decimals,
        }
    }
}

#[derive(Display, Validate, Debug, Clone, PartialEq, Eq, Hash)]
#[display("{}/{}", base, quote)]
pub struct Instrument {
    /// Base asset (e.g., BTC)
    pub base: Asset,
    /// Quote asset (e.g., USDT)
    pub quote: Asset,
}
impl Instrument {
    pub fn new(base: Asset, quote: Asset) -> Self {
        Self { base, quote }
    }
}

/// Represents the side of an order in the order book.
///
/// Orders can be either buy orders (bids) or sell orders (asks).
#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "cli", value(rename_all = "lower"))]
pub enum Side {
    /// Buy order (bid) - willing to buy at specified price or lower
    Buy,
    /// Sell order (ask) - willing to sell at specified price or higher
    Sell,
}

/// Represents an order in the order book.
///
/// An order contains all the information needed to match and execute trades,
/// including the order ID, side (buy/sell), price, quantity, and timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// Unique identifier for the order
    pub id: Id,
    /// Whether this is a buy or sell order
    pub side: Side,
    /// Price per unit in the smallest denomination
    pub price: Price,
    /// Number of units to buy or sell
    pub quantity: Quantity,
    /// Unix timestamp when the order was created
    pub timestamp: Timestamp,
}

impl Order {
    /// Creates a new order with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the order
    /// * `side` - Whether this is a buy or sell order
    /// * `price` - Price per unit
    /// * `quantity` - Number of units to trade
    /// * `timestamp` - Unix timestamp when the order was created
    pub fn new(id: Id, side: Side, price: Price, quantity: Quantity, timestamp: Timestamp) -> Self {
        Order {
            id,
            side,
            price,
            quantity,
            timestamp,
        }
    }
}

/// Represents a completed trade between two orders.
///
/// A trade occurs when a buy and sell order match at an agreed price.
/// The maker is the order that was resting in the book, while the taker
/// is the order that matched against it.
#[derive(Display, Debug, Clone, PartialEq, Eq)]
#[display(
    "Trade: {} @ {} (maker: {}, taker: {})",
    quantity,
    price,
    maker_id,
    taker_id
)]
pub struct Trade {
    /// Execution price of the trade
    pub price: Price,
    /// Number of units traded
    pub quantity: Quantity,
    /// ID of the maker order (resting in book)
    pub maker_id: Id,
    /// ID of the taker order (incoming)
    pub taker_id: Id,
}

impl Trade {
    /// Creates a new trade record.
    ///
    /// # Arguments
    ///
    /// * `price` - Execution price of the trade
    /// * `quantity` - Number of units traded
    /// * `maker_id` - ID of the maker order
    /// * `taker_id` - ID of the taker order
    pub fn new(price: Price, quantity: Quantity, maker_id: Id, taker_id: Id) -> Self {
        Trade {
            price,
            quantity,
            maker_id,
            taker_id,
        }
    }
}
/// A collection of trades, typically returned from order matching operations.
pub type Trades = Vec<Trade>;

/// Error type for order book operations
#[derive(Display, Debug, Clone, PartialEq, Eq)]
pub enum OrderBookError {
    /// Order ID already exists in the book
    #[display("Order {} already in book", 0)]
    DuplicateOrderId(Id),
    /// Order quantity is zero
    #[display("Order {} quantity {} is 0, no order placed", id, quantity)]
    ZeroQuantity { id: Id, quantity: Quantity },
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Asset ----------

    #[test]
    fn asset_display_and_new() {
        let btc = Asset::new("BTC", 8);
        assert_eq!(format!("{}", btc), "BTC");
        assert_eq!(btc.symbol, "BTC");
        assert_eq!(btc.decimals, 8);

        let usdt = Asset::new("USDT", 2);
        assert_eq!(format!("{}", usdt), "USDT");
        assert_eq!(usdt.decimals, 2);
    }

    // ---------- PriceLevel (with your Order) ----------

    fn mk_order(id: Id, qty: Quantity) -> Order {
        // Side/price/timestamp don't matter for PriceLevel behavior; choose placeholders.
        Order::new(id, Side::Buy, 0, qty, 0)
    }

    #[test]
    fn price_level_new_and_is_empty() {
        let mut lvl = PriceLevel::new(10);
        assert_eq!(lvl.price, 10);
        assert!(lvl.is_empty());
        assert_eq!(lvl.total_quantity, 0);

        lvl.add_order(mk_order(1, 5));
        assert!(!lvl.is_empty());
        assert_eq!(lvl.total_quantity, 5);
    }

    #[test]
    fn price_level_add_fifo_and_totals() {
        let mut lvl = PriceLevel::new(42);

        let o1 = mk_order(1, 30);
        let o2 = mk_order(2, 20);

        lvl.add_order(o1.clone());
        lvl.add_order(o2.clone());

        assert_eq!(lvl.orders.len(), 2);
        // FIFO preserved
        assert_eq!(lvl.orders.front().unwrap().id, o1.id);
        assert_eq!(lvl.orders.back().unwrap().id, o2.id);
        assert_eq!(lvl.total_quantity, 50);
    }

    #[test]
    fn price_level_remove_and_update_front() {
        let mut lvl = PriceLevel::new(99);

        lvl.add_order(mk_order(1, 10));
        lvl.add_order(mk_order(2, 25));

        // Partial fill of front order: 10 -> 4
        lvl.update_front_order_quantity(4);
        assert_eq!(lvl.orders.front().unwrap().quantity, 4);
        assert_eq!(lvl.total_quantity, 4 + 25);

        // Remove front (id=1)
        let removed = lvl.remove_order().expect("has front");
        assert_eq!(removed.id, 1);
        assert_eq!(removed.quantity, 4);
        assert_eq!(lvl.total_quantity, 25);
        assert_eq!(lvl.orders.front().unwrap().id, 2);

        // Remove last
        let removed2 = lvl.remove_order().expect("has second");
        assert_eq!(removed2.id, 2);
        assert_eq!(lvl.total_quantity, 0);
        assert!(lvl.is_empty());

        // Removing from empty => None
        assert!(lvl.remove_order().is_none());
    }
}
