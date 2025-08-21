# Limit Order Book Implementation

A high-performance, production-ready limit order book implementation in Rust with price-time priority matching.

## Features

- **Price-Time Priority**: Orders are matched by best price first, then by arrival time (FIFO)
- **Partial Fills**: Orders can be partially filled with remainder staying in the book
- **Comprehensive Error Handling**: Type-safe error handling with detailed error types
- **Production Ready**: Uses minor units to avoid floating-point precision issues
- **Clean Architecture**: Core engine deals with integers, CLI provides decimal interface

## Project Structure

```
├── order-book-core/    # Core order book library
├── order-book-cli/     # Command-line interface
└── demo/               # Interactive demonstration
```

## Quick Start

### TL;DR - Start Trading

```bash
# Default BTC/USDT trading (no arguments needed)
cargo run --bin order-book-cli

# Custom ETH/USD trading (specify assets explicitly)
cargo run --bin order-book-cli -- --base-asset ETH --quote-asset USD
```

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))

### Build the Project

```bash
cargo build --release
```

### Run Tests

```bash
# Run all tests
cargo test --workspace

# Run with output
cargo test --workspace -- --nocapture
```

## Demo

The demo showcases all order book functionality with real-world scenarios.

### Run the Demo

```bash
cargo run --bin demo
```

### Demo Scenarios

1. **Basic Matching**: Demonstrates exact price matching between buy and sell orders
2. **Partial Fills**: Shows partial order execution with remainder handling
3. **Price-Time Priority**: Illustrates best price matching, then FIFO within price levels
4. **Complex Market**: Multi-level order matching across the spread

### Example Output

```
=== Limit Order Book Demo ===

Instrument details: BTC/USDT
-----------------------
1. Basic Matching Demo:
-----------------------
--Placing Buy order: ID=1, Price=100.00, Qty=0.010
--No trades executed
--Book state:
----Best BUY:  0.01 BTC @ 100 USDT
----Best SELL: None

--Placing Sell order: ID=2, Price=100.00, Qty=0.010
--Trades executed:
----Trade: 0.01 BTC @ 100 USDT (maker: 1, taker: 2)
```

## CLI Usage

The CLI defaults to **interactive mode** for the best user experience, but also supports individual commands for scripting.

### Run the CLI (Interactive Mode - Default)

```bash
# Start interactive mode (defaults to BTC/USDT pair)
cargo run --bin order-book-cli

# Or explicitly specify interactive mode (still defaults to BTC/USDT)
cargo run --bin order-book-cli -- interactive

# Use custom assets (ETH/USD example)
cargo run --bin order-book-cli -- --base-asset ETH --base-decimals 4 --quote-asset USD --quote-decimals 2
```

### Asset Configuration

The CLI supports configurable trading pairs. **By default, it uses BTC/USDT** (BTC with 6 decimals, USDT with 2 decimals). You can customize using these options:

- `--base-asset`: The asset being traded (default: `BTC`)
- `--base-decimals`: Decimal places for base asset (default: `6` for satoshis) 
- `--quote-asset`: The pricing asset (default: `USDT`)
- `--quote-decimals`: Decimal places for quote asset (default: `2` for cents)

**Examples:**
- BTC/USDT (default): `--base-asset BTC --base-decimals 6 --quote-asset USDT --quote-decimals 2`
- ETH/USD: `--base-asset ETH --base-decimals 4 --quote-asset USD --quote-decimals 2`
- DOGE/BTC: `--base-asset DOGE --base-decimals 8 --quote-asset BTC --quote-decimals 8`

### Interactive Mode Commands

Interactive mode starts automatically and provides persistent order book state throughout your session.

**Available commands:**
- `buy <price> <quantity> [id]` - Place a buy order (e.g., `buy 100.50 0.001`)
- `sell <price> <quantity> [id]` - Place a sell order (e.g., `sell 100.25 0.0015`)  
- `book` (or `state`, `b`) - Show current order book state
- `best` - Show best bid and ask prices
- `depth [levels]` - Show market depth (default: 5 levels)
- `clear` - Clear the order book
- `help` (or `h`) - Show help message
- `quit` (or `exit`, `q`) - Exit the CLI

### CLI Examples

```bash
# Interactive mode with default BTC/USDT
$ cargo run --bin order-book-cli
=== Valhalla Order Book Interactive CLI ===
Type 'help' for available commands, 'quit' to exit

Instrument: BTC/USDT

> buy 100.50 0.001
✅ Order 1 placed. No trades executed.
📊 Best: 0.001 BTC @ 100.50 USDT | No asks
> sell 100.50 0.0005
🎯 Order 2 executed! Trades:
  💰 Trade: 0.0005 BTC @ 100.50 USDT (maker: 1, taker: 2)
📊 Best: 0.0005 BTC @ 100.50 USDT | No asks
> quit
Goodbye!

# Interactive mode with custom ETH/USD pair
$ cargo run --bin order-book-cli -- --base-asset ETH --quote-asset USD
=== Valhalla Order Book Interactive CLI ===
Type 'help' for available commands, 'quit' to exit

Instrument: ETH/USD

> buy 1500.00 0.1
✅ Order 1 placed. No trades executed.
📊 Best: 0.1 ETH @ 1500.00 USD | No asks
> quit
Goodbye!
```

### Help

```bash
# Show CLI options and asset configuration
cargo run --bin order-book-cli -- --help

# Get help within interactive mode (recommended)
cargo run --bin order-book-cli
# Then type 'help' in the interactive prompt
```

## Core Library API

The core library provides the following main types and functions:

```rust
use order_book_core::{OrderBook, Side};
use order_book_core::types::{Asset, Instrument};

// Create an instrument (BTC/USDT)
let btc = Asset::new("BTC", 6);  // 6 decimal places (satoshis)
let usdt = Asset::new("USDT", 2); // 2 decimal places (cents)
let instrument = Instrument::new(btc, usdt);

// Create order book
let mut book = OrderBook::new(instrument);

// Place orders (prices and quantities in minor units)
// Price: 100.50 USDT = 10050 (price * 10^2)
// Quantity: 0.001 BTC = 1000 (quantity * 10^6)
let trades = book.place_order(Side::Buy, 10050, 1000, 1)?;

// Query best prices
let best_buy = book.best_buy();  // Option<(price, total_quantity)>
let best_sell = book.best_sell(); // Option<(price, total_quantity)>
```

## Architecture

### Clean Separation of Concerns

- **Core Engine**: Operates purely on integer values (minor units) for precision
- **CLI Layer**: Handles decimal input/output conversion using helper functions
- **Demo**: Shows real-world usage with proper formatting

### Minor Units System

The system represents each currency in its smallest unit to avoid floating-point precision issues:

- **BTC**: Represented in satoshis (6 decimal places)
  - 1 BTC = 1,000,000 satoshis
  - 0.001 BTC = 1,000 satoshis
- **USDT**: Represented in cents (2 decimal places) 
  - $100.50 = 10,050 cents
  - $1.00 = 100 cents

### Helper Functions

The core library provides conversion helpers:

```rust
use order_book_core::{
    price_to_minor_units, quantity_to_minor_units,
    format_price, format_quantity
};

// Convert decimal to minor units
let price_minor = price_to_minor_units(Decimal::from_str("100.50")?, &usdt)?;
let qty_minor = quantity_to_minor_units(Decimal::from_str("0.001")?, &btc)?;

// Format minor units for display
let price_str = format_price(10050, &usdt); // "100.50 USDT"
let qty_str = format_quantity(1000, &btc);  // "0.001 BTC"
```

## Performance

- **O(log n)** order insertion and removal using `BTreeMap`
- **O(1)** order ID lookups using `HashSet`
- Efficient FIFO queue per price level using `VecDeque`
- Zero-copy operations where possible

## Architecture Highlights

- **Type Safety**: Strong typing with `Price`, `Quantity`, and `Id` types
- **Error Handling**: Comprehensive `Result` types with detailed error variants
- **Minor Units**: All prices and quantities stored as integers to avoid floating-point issues
- **Modular Design**: Clean separation between core logic, decimal conversion, and UI
- **User-Friendly**: CLI accepts natural decimal inputs while core maintains precision

## Testing

The project includes comprehensive test coverage:

- Unit tests for all core functionality
- Integration tests for the CLI
- Edge case testing (zero quantity, duplicate IDs)
- Performance testing with large order books
- Decimal conversion and formatting tests

Run specific test suites:

```bash
# Core library tests only
cargo test -p order-book-core

# CLI tests only  
cargo test -p order-book-cli

# Run a specific test
cargo test test_price_time_priority
```

## License

This project is provided as-is for technical interview purposes.