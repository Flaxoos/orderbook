//! # Order Book CLI
//!
//! A command-line interface for interacting with the order book.
//!
//! This CLI provides commands to place orders, query book state, and run an interactive mode.

use clap::{Parser, Subcommand};
use order_book_core::{
    OrderBook, Side,
    format_price, format_quantity, price_to_minor_units, quantity_to_minor_units
};
use order_book_core::types::{Asset, Instrument};
use rust_decimal::Decimal;
use std::io::{self, Write};
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "order-book-cli")]
#[command(about = "A limit order book CLI", long_about = None)]
struct Cli {
    /// Base asset symbol (e.g., BTC)
    #[arg(long, default_value = "BTC")]
    base_asset: String,
    
    /// Base asset decimals (e.g., 6 for BTC satoshis)  
    #[arg(long, default_value = "6")]
    base_decimals: u8,
    
    /// Quote asset symbol (e.g., USDT)
    #[arg(long, default_value = "USDT")]  
    quote_asset: String,
    
    /// Quote asset decimals (e.g., 2 for USDT cents)
    #[arg(long, default_value = "2")]
    quote_decimals: u8,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Place an order in the book
    #[command(name = "place-order")]
    PlaceOrder {
        /// Order side (buy/sell)
        side: Side,
        /// Price in decimal format (e.g., 100.50)
        price: String,
        /// Quantity in decimal format (e.g., 0.001)
        quantity: String,
        /// Unique order ID
        id: u64,
    },
    /// Place a buy order (interactive mode)
    #[command(name = "buy")]
    Buy {
        /// Price in decimal format (e.g., 100.50)
        price: String,
        /// Quantity in decimal format (e.g., 0.001)
        quantity: String,
        /// Unique order ID (auto-generated if not provided)
        id: Option<u64>,
    },
    /// Place a sell order (interactive mode)
    #[command(name = "sell")]
    Sell {
        /// Price in decimal format (e.g., 100.50)
        price: String,
        /// Quantity in decimal format (e.g., 0.001)
        quantity: String,
        /// Unique order ID (auto-generated if not provided)
        id: Option<u64>,
    },
    /// Show current order book state
    #[command(name = "book", aliases = ["state", "b"])]
    Book,
    /// Show best bid and ask prices
    #[command(name = "best")]
    Best,
    /// Get the best buy price and quantity
    #[command(name = "best-buy")]
    BestBuy,
    /// Get the best sell price and quantity  
    #[command(name = "best-sell")]
    BestSell,
    /// Show market depth
    #[command(name = "depth")]
    Depth {
        /// Number of levels to show (default: 5)
        #[arg(default_value = "5")]
        levels: usize,
    },
    /// Clear the order book (interactive mode)
    #[command(name = "clear")]
    Clear,
    /// Exit interactive mode
    #[command(name = "quit", aliases = ["exit", "q"])]
    Quit,
    /// Start interactive mode
    #[command(name = "interactive")]
    Interactive,
}

fn main() {
    let cli = Cli::parse();

    // Create instrument from CLI arguments
    let base_asset = Asset { symbol: cli.base_asset.into(), decimals: cli.base_decimals };
    let quote_asset = Asset { symbol: cli.quote_asset.into(), decimals: cli.quote_decimals };
    let instrument = Instrument::new(base_asset, quote_asset);

    match cli.command {
        None => {
            // Default to interactive mode when no command is provided
            run_interactive_mode(instrument);
        }
        Some(Commands::PlaceOrder { side, price, quantity, id }) => {
            let mut book = OrderBook::new(instrument);
            match place_order(&mut book, side, &price, &quantity, id) {
                Ok(trades) => {
                    if trades.is_empty() {
                        println!("Order placed. No trades executed.");
                    } else {
                        println!("Order executed! Trades:");
                        for trade in &trades {
                            let price_str = format_price(trade.price, &book.instrument.quote);
                            let qty_str = format_quantity(trade.quantity, &book.instrument.base);
                            println!("Trade: {} @ {} (maker: {}, taker: {})",
                                qty_str, price_str, trade.maker_id, trade.taker_id);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error placing order: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::BestBuy) => {
            let book = OrderBook::new(instrument);
            match book.best_buy() {
                Some((price, quantity)) => {
                    let price_str = format_price(price, &book.instrument.quote);
                    let qty_str = format_quantity(quantity, &book.instrument.base);
                    println!("Best buy: {} @ {}", qty_str, price_str);
                }
                None => println!("No buy orders"),
            }
        }
        Some(Commands::BestSell) => {
            let book = OrderBook::new(instrument);
            match book.best_sell() {
                Some((price, quantity)) => {
                    let price_str = format_price(price, &book.instrument.quote);
                    let qty_str = format_quantity(quantity, &book.instrument.base);
                    println!("Best sell: {} @ {}", qty_str, price_str);
                }
                None => println!("No sell orders"),
            }
        }
        Some(Commands::Interactive) => {
            run_interactive_mode(instrument);
        }
        // These commands are only used in interactive mode
        Some(Commands::Buy { .. }) | Some(Commands::Sell { .. }) | Some(Commands::Book) | 
        Some(Commands::Best) | Some(Commands::Depth { .. }) | Some(Commands::Clear) | 
        Some(Commands::Quit) => {
            eprintln!("This command is only available in interactive mode.");
            eprintln!("Use: cargo run --bin order-book-cli -- interactive");
            std::process::exit(1);
        }
    }
}

/// Parse interactive command using clap
fn parse_interactive_command(input: &str) -> Result<Commands, String> {
    // Split the input into arguments, handling quotes properly
    let args = shlex::split(input).ok_or("Invalid command syntax")?;
    if args.is_empty() {
        return Err("Empty command".to_string());
    }
    
    // Prepend a dummy program name for clap parsing
    let mut full_args = vec!["order-book-cli".to_string()];
    full_args.extend(args);
    
    // Parse using clap
    match Cli::try_parse_from(full_args) {
        Ok(cli) => match cli.command {
            Some(command) => Ok(command),
            None => Err("Interactive mode not available within interactive mode".to_string()),
        },
        Err(e) => Err(e.to_string()),
    }
}

/// Runs the interactive REPL mode
fn run_interactive_mode(instrument: Instrument) {
    println!("=== Order Book Interactive CLI ===");
    println!("Type 'help' for available commands, 'quit' to exit\n");

    let mut book = OrderBook::new(instrument);

    println!("Instrument: {}\n", book.instrument);

    let mut next_id = 1u64;

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match parse_interactive_command(trimmed) {
                    Ok(command) => {
                        match command {
                            Commands::Quit => {
                                println!("Goodbye!");
                                break;
                            }
                            Commands::Buy { price, quantity, id } => {
                                let order_id = id.unwrap_or_else(|| {
                                    let id = next_id;
                                    next_id += 1;
                                    id
                                });
                                
                                match place_order(&mut book, Side::Buy, &price, &quantity, order_id) {
                                    Ok(trades) => {
                                        if trades.is_empty() {
                                            println!("✅ Order {} placed. No trades executed.", order_id);
                                        } else {
                                            println!("🎯 Order {} executed! Trades:", order_id);
                                            for trade in &trades {
                                                let price_str = format_price(trade.price, &book.instrument.quote);
                                                let qty_str = format_quantity(trade.quantity, &book.instrument.base);
                                                println!("  💰 Trade: {} @ {} (maker: {}, taker: {})",
                                                    qty_str, price_str, trade.maker_id, trade.taker_id);
                                            }
                                        }
                                        print_book_summary(&book);
                                    }
                                    Err(e) => println!("❌ Error: {}", e),
                                }
                            }
                            Commands::Sell { price, quantity, id } => {
                                let order_id = id.unwrap_or_else(|| {
                                    let id = next_id;
                                    next_id += 1;
                                    id
                                });
                                
                                match place_order(&mut book, Side::Sell, &price, &quantity, order_id) {
                                    Ok(trades) => {
                                        if trades.is_empty() {
                                            println!("✅ Order {} placed. No trades executed.", order_id);
                                        } else {
                                            println!("🎯 Order {} executed! Trades:", order_id);
                                            for trade in &trades {
                                                let price_str = format_price(trade.price, &book.instrument.quote);
                                                let qty_str = format_quantity(trade.quantity, &book.instrument.base);
                                                println!("  💰 Trade: {} @ {} (maker: {}, taker: {})",
                                                    qty_str, price_str, trade.maker_id, trade.taker_id);
                                            }
                                        }
                                        print_book_summary(&book);
                                    }
                                    Err(e) => println!("❌ Error: {}", e),
                                }
                            }
                            Commands::Book => print_book_state(&book),
                            Commands::Best => print_best_prices(&book),
                            Commands::Clear => {
                                let instrument = book.instrument.clone();
                                book = OrderBook::new(instrument);
                                next_id = 1;
                                println!("📝 Order book cleared.");
                            }
                            Commands::Depth { levels } => {
                                print_market_depth(&book, levels);
                            }
                            // These commands shouldn't be available in interactive mode
                            Commands::PlaceOrder { .. } | Commands::BestBuy | Commands::BestSell | Commands::Interactive => {
                                println!("❌ Command not available in interactive mode.");
                            }
                        }
                    }
                    Err(e) => {
                        // Handle help commands specially
                        if trimmed.trim() == "help" || trimmed.trim() == "h" {
                            show_help();
                        } else if e.contains("unexpected argument") || e.contains("invalid value") {
                            println!("❌ Invalid command. Type 'help' for available commands.");
                        } else if e.contains("required arguments") || e.contains("The following required arguments") {
                            println!("❌ Missing required arguments. Type 'help' for usage.");
                        } else {
                            println!("❌ Error: {}", e.lines().next().unwrap_or("Invalid command"));
                        }
                    }
                }
            }
            Err(error) => {
                println!("Error reading input: {}", error);
                break;
            }
        }
    }
}

fn show_help() {
    println!("📚 Available Commands:");
    println!("  buy <price> <quantity> [id]    - Place a buy order (e.g., buy 100.50 0.001)");
    println!("  sell <price> <quantity> [id]   - Place a sell order (e.g., sell 100.25 0.0015)");
    println!("  book | state | b               - Show current order book state");
    println!("  best                           - Show best bid and ask prices");
    println!("  depth [levels]                 - Show market depth (default: 5 levels)");
    println!("  clear                          - Clear the order book");
    println!("  help | h                       - Show this help message");
    println!("  quit | exit | q                - Exit the CLI");
    println!();
    println!("💡 Tips:");
    println!("  - Prices and quantities use decimal format (e.g., 100.50, 0.001)");
    println!("  - IDs are auto-generated if not provided");
    println!("  - Orders are matched using price-time priority");
    println!("  - All commands support clap-style arguments and help (e.g., 'buy --help')");
    println!();
}

fn place_order(
    book: &mut OrderBook,
    side: Side,
    price_str: &str,
    quantity_str: &str,
    id: u64,
) -> Result<Vec<order_book_core::Trade>, String> {
    // Parse decimal strings
    let price_decimal = Decimal::from_str(price_str)
        .map_err(|_| format!("Invalid price format: {}", price_str))?;
    let quantity_decimal = Decimal::from_str(quantity_str)
        .map_err(|_| format!("Invalid quantity format: {}", quantity_str))?;

    // Convert to minor units using asset decimals
    let price_minor = price_to_minor_units(price_decimal, &book.instrument.quote)
        .ok_or("Price too large to convert to minor units")?;
    let quantity_minor = quantity_to_minor_units(quantity_decimal, &book.instrument.base)
        .ok_or("Quantity too large to convert to minor units")?;

    book.place_order(side, price_minor, quantity_minor, id)
        .map_err(|e| e.to_string())
}

fn print_book_state(book: &OrderBook) {
    println!("\n📊 Order Book State:");

    // Show best prices
    print_best_prices(book);

    // Show some market depth
    print_market_depth(book, 3);
    println!();
}

fn print_best_prices(book: &OrderBook) {
    match (book.best_buy(), book.best_sell()) {
        (Some((buy_price, buy_qty)), Some((sell_price, sell_qty))) => {
            let buy_price_str = format_price(buy_price, &book.instrument.quote);
            let buy_qty_str = format_quantity(buy_qty, &book.instrument.base);
            let sell_price_str = format_price(sell_price, &book.instrument.quote);
            let sell_qty_str = format_quantity(sell_qty, &book.instrument.base);

            let spread = sell_price - buy_price;
            let spread_str = format_price(spread, &book.instrument.quote);

            println!("  💚 Best BUY:  {} @ {}", buy_qty_str, buy_price_str);
            println!("  ❤️  Best SELL: {} @ {}", sell_qty_str, sell_price_str);
            println!("  📏 Spread:    {}", spread_str);
        }
        (Some((buy_price, buy_qty)), None) => {
            let buy_price_str = format_price(buy_price, &book.instrument.quote);
            let buy_qty_str = format_quantity(buy_qty, &book.instrument.base);
            println!("  💚 Best BUY:  {} @ {}", buy_qty_str, buy_price_str);
            println!("  ❤️  Best SELL: None");
        }
        (None, Some((sell_price, sell_qty))) => {
            let sell_price_str = format_price(sell_price, &book.instrument.quote);
            let sell_qty_str = format_quantity(sell_qty, &book.instrument.base);
            println!("  💚 Best BUY:  None");
            println!("  ❤️  Best SELL: {} @ {}", sell_qty_str, sell_price_str);
        }
        (None, None) => {
            println!("  📭 Order book is empty");
        }
    }
}

fn print_market_depth(book: &OrderBook, levels: usize) {
    let buy_depth = book.depth(Side::Buy, levels);
    let sell_depth = book.depth(Side::Sell, levels);

    if !sell_depth.is_empty() || !buy_depth.is_empty() {
        println!("  📈 Market Depth:");

        // Print sell side (asks) in reverse order (highest first)
        for (price, qty) in sell_depth.iter().rev() {
            let price_str = format_price(*price, &book.instrument.quote);
            let qty_str = format_quantity(*qty, &book.instrument.base);
            println!("    🔴 {} @ {}", qty_str, price_str);
        }

        if !sell_depth.is_empty() && !buy_depth.is_empty() {
            println!("    ─────────────────");
        }

        // Print buy side (bids) in normal order (highest first)
        for (price, qty) in &buy_depth {
            let price_str = format_price(*price, &book.instrument.quote);
            let qty_str = format_quantity(*qty, &book.instrument.base);
            println!("    🟢 {} @ {}", qty_str, price_str);
        }
    }
}

fn print_book_summary(book: &OrderBook) {
    match (book.best_buy(), book.best_sell()) {
        (Some((buy_price, buy_qty)), Some((sell_price, sell_qty))) => {
            let buy_price_str = format_price(buy_price, &book.instrument.quote);
            let buy_qty_str = format_quantity(buy_qty, &book.instrument.base);
            let sell_price_str = format_price(sell_price, &book.instrument.quote);
            let sell_qty_str = format_quantity(sell_qty, &book.instrument.base);
            println!("📊 Best: {} @ {} | {} @ {}",
                buy_qty_str, buy_price_str, sell_qty_str, sell_price_str);
        }
        (Some((buy_price, buy_qty)), None) => {
            let buy_price_str = format_price(buy_price, &book.instrument.quote);
            let buy_qty_str = format_quantity(buy_qty, &book.instrument.base);
            println!("📊 Best: {} @ {} | No asks", buy_qty_str, buy_price_str);
        }
        (None, Some((sell_price, sell_qty))) => {
            let sell_price_str = format_price(sell_price, &book.instrument.quote);
            let sell_qty_str = format_quantity(sell_qty, &book.instrument.base);
            println!("📊 Best: No bids | {} @ {}", sell_qty_str, sell_price_str);
        }
        (None, None) => {
            println!("📊 Order book is empty");
        }
    }
}


#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    
    fn get_cli_command() -> Command {
        Command::cargo_bin("order-book-cli").unwrap_or_else(|e| {
            panic!("CLI binary not found. Please run 'cargo build --bin order-book-cli' first.\nOriginal error: {}", e);
        })
    }
    #[test]
    fn test_place_buy_order_no_match() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "100", "10", "1"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Order placed. No trades executed."));
    }

    #[test]
    fn test_place_sell_order_no_match() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "sell", "100", "10", "1"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Order placed. No trades executed."));
    }

    #[test]
    fn test_best_buy_empty_book() {
        let mut cmd = get_cli_command();
        cmd.arg("best-buy")
            .assert()
            .success()
            .stdout(predicate::str::contains("No buy orders"));
    }

    #[test]
    fn test_best_sell_empty_book() {
        let mut cmd = get_cli_command();
        cmd.arg("best-sell")
            .assert()
            .success()
            .stdout(predicate::str::contains("No sell orders"));
    }

    #[test]
    fn test_case_sensitive_side() {
        // Test that uppercase side values are rejected
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "BUY", "100", "10", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid value"));

        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "SELL", "100", "10", "2"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid value"));
    }

    #[test]
    fn test_invalid_side() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "invalid", "100", "10", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_invalid_price() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "not_a_number", "10", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Error placing order"));
    }

    #[test]
    fn test_invalid_quantity() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "100", "not_a_number", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Error placing order"));
    }

    #[test]
    fn test_invalid_id() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "100", "10", "not_a_number"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_missing_arguments() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_help_command() {
        let mut cmd = get_cli_command();
        cmd.arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("A limit order book CLI"))
            .stdout(predicate::str::contains("Commands:"))
            .stdout(predicate::str::contains("place-order"))
            .stdout(predicate::str::contains("best-buy"))
            .stdout(predicate::str::contains("best-sell"));
    }

    #[test]
    fn test_version_flag() {
        let mut cmd = get_cli_command();
        cmd.arg("--version")
            .assert()
            .failure()
            .stderr(predicate::str::contains("unexpected argument"));
    }

    #[test]
    fn test_no_subcommand_starts_interactive() {
        let mut cmd = get_cli_command();
        cmd.write_stdin("quit\n")
            .assert()
            .success()
            .stdout(predicate::str::contains("=== Order Book Interactive CLI ==="));
    }

    #[test]
    fn test_unknown_subcommand() {
        let mut cmd = get_cli_command();
        cmd.arg("unknown")
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_place_order_help() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Arguments:"))
            .stdout(predicate::str::contains("<SIDE>"))
            .stdout(predicate::str::contains("<PRICE>"))
            .stdout(predicate::str::contains("<QUANTITY>"))
            .stdout(predicate::str::contains("<ID>"));
    }

    #[test]
    fn test_negative_price() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "-100", "10", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_negative_quantity() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "100", "-10", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_large_numbers() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "1000000000", "1000000000", "1000000000"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Order placed. No trades executed."));
    }

    #[test]
    fn test_zero_quantity() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "100", "0", "1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Error placing order"));
    }

    #[test]
    fn test_zero_price() {
        let mut cmd = get_cli_command();
        cmd.args(&["place-order", "buy", "0", "10", "1"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Order placed. No trades executed."));
    }
}