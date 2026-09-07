use limit_order_book::simulator::{
    GeneratorConfig, ScenarioCommand, ScenarioResult, generate_crossing_orders,
    generate_two_sided_orders, run_scenario, scenarios,
};
use limit_order_book::{
    BookEvent, BookMetrics, Trade, TradeMetrics, calculate_book_metrics, calculate_trade_metrics,
    load_events_from_file, replay_events, save_events_to_file,
};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
};

const DEFAULT_SCENARIO: &str = "simple-cross";
const DEFAULT_SYNTHETIC_COUNT: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CliConfig::from_args(env::args().collect());

    if let Some(path) = &config.replay_events_path {
        run_replay_mode(path)?;
        return Ok(());
    }

    let commands = commands_for_scenario(&config.scenario_name, config.synthetic_count);
    let result = run_scenario(&commands)?;
    let book_metrics = calculate_book_metrics(&result.book.snapshot());
    let trade_metrics = calculate_trade_metrics(&result.trades);

    write_optional_artifacts(&config, &result, &book_metrics, &trade_metrics)?;
    print_run_output(&config, &result, &book_metrics, &trade_metrics)?;

    Ok(())
}

struct CliConfig {
    scenario_name: String,
    synthetic_count: usize,
    output_json: bool,
    output_dir: Option<PathBuf>,
    save_events_path: Option<PathBuf>,
    replay_events_path: Option<PathBuf>,
}

impl CliConfig {
    fn from_args(args: Vec<String>) -> Self {
        Self {
            scenario_name: scenario_name(&args),
            synthetic_count: synthetic_count(&args),
            output_json: args.iter().any(|arg| arg == "--json"),
            output_dir: option_path_arg(&args, "--output-dir"),
            save_events_path: option_path_arg(&args, "--save-events"),
            replay_events_path: option_path_arg(&args, "--replay-events"),
        }
    }
}

fn scenario_name(args: &[String]) -> String {
    // First positional argument is the scenario name.
    // Example: cargo run -- buy-sweeps-asks
    args.get(1)
        .cloned()
        .unwrap_or_else(|| DEFAULT_SCENARIO.to_string())
}

fn synthetic_count(args: &[String]) -> usize {
    // Optional synthetic order count.
    // Example: cargo run -- synthetic --count 1000
    args.windows(2)
        .find(|window| window[0] == "--count")
        .and_then(|window| window[1].parse::<usize>().ok())
        .unwrap_or(DEFAULT_SYNTHETIC_COUNT)
}

fn option_path_arg(args: &[String], flag: &str) -> Option<PathBuf> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| PathBuf::from(&window[1]))
}

fn commands_for_scenario(scenario_name: &str, synthetic_count: usize) -> Vec<ScenarioCommand> {
    match scenario_name {
        "simple-cross" => scenarios::simple_cross(),
        "buy-sweeps-asks" => scenarios::buy_sweeps_asks(),
        "cancel-and-modify" => scenarios::cancel_and_modify_flow(),
        "two-sided-book" => scenarios::two_sided_book(),
        "synthetic" => generate_two_sided_orders(default_generator_config(synthetic_count)),
        "synthetic-crossing" => generate_crossing_orders(default_generator_config(synthetic_count)),
        _ => exit_unknown_scenario(scenario_name),
    }
}

fn default_generator_config(order_count: usize) -> GeneratorConfig {
    GeneratorConfig {
        order_count,
        start_order_id: 1,
        base_price: 100,
        tick_size: 1,
        price_levels: 10,
        quantity: 5,
    }
}

fn exit_unknown_scenario(scenario_name: &str) -> ! {
    eprintln!("unknown scenario: {scenario_name}");
    eprintln!("available scenarios:");
    eprintln!("  simple-cross");
    eprintln!("  buy-sweeps-asks");
    eprintln!("  cancel-and-modify");
    eprintln!("  two-sided-book");
    eprintln!("  synthetic");
    eprintln!("  synthetic-crossing");
    std::process::exit(1);
}

fn run_replay_mode(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let events = load_events_from_file(path)?;
    let book = replay_events(&events)?;
    let book_metrics = calculate_book_metrics(&book.snapshot());
    let trade_metrics = calculate_trade_metrics(&trades_from_events(&events));

    println!("replayed events from: {}", path.display());
    println!("best bid: {:?}", book.best_bid());
    println!("best ask: {:?}", book.best_ask());
    println!("resting orders: {}", book.resting_order_count());
    println!("snapshot: {:?}", book.snapshot());
    println!("book metrics: {:?}", book_metrics);
    println!("trade metrics: {:?}", trade_metrics);

    Ok(())
}

fn trades_from_events(events: &[BookEvent]) -> Vec<Trade> {
    events
        .iter()
        .filter_map(|event| match event {
            BookEvent::TradeExecuted { trade } => Some(trade.clone()),
            _ => None,
        })
        .collect()
}

fn write_optional_artifacts(
    config: &CliConfig,
    result: &ScenarioResult,
    book_metrics: &BookMetrics,
    trade_metrics: &TradeMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(output_dir) = &config.output_dir {
        write_run_artifacts(config, result, book_metrics, trade_metrics, output_dir)?;
    }

    if let Some(path) = &config.save_events_path {
        save_events_to_file(&result.events, path)?;

        // Keep JSON mode clean so tools can parse stdout.
        if !config.output_json {
            println!("saved events to: {}", path.display());
        }
    }

    Ok(())
}

fn write_run_artifacts(
    config: &CliConfig,
    result: &ScenarioResult,
    book_metrics: &BookMetrics,
    trade_metrics: &TradeMetrics,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;

    save_events_to_file(&result.events, output_dir.join("events.json"))?;
    write_json_file(output_dir.join("snapshot.json"), &result.book.snapshot())?;
    write_json_file(
        output_dir.join("summary.json"),
        &summary_json(config, result, book_metrics, trade_metrics),
    )?;

    if !config.output_json {
        println!("saved run artifacts to: {}", output_dir.display());
    }

    Ok(())
}

fn print_run_output(
    config: &CliConfig,
    result: &ScenarioResult,
    book_metrics: &BookMetrics,
    trade_metrics: &TradeMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    if config.output_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&run_json(config, result, book_metrics, trade_metrics))?
        );
        return Ok(());
    }

    for event in &result.events {
        println!("  {event:?}");
    }

    println!("scenario: {}", config.scenario_name);
    println!("trades: {:?}", result.trades);
    println!("best bid: {:?}", result.book.best_bid());
    println!("best ask: {:?}", result.book.best_ask());
    println!("resting orders: {}", result.book.resting_order_count());
    println!("snapshot: {:?}", result.book.snapshot());
    println!("metrics: {:?}", book_metrics);
    println!("trade metrics: {:?}", trade_metrics);

    Ok(())
}

fn summary_json(
    config: &CliConfig,
    result: &ScenarioResult,
    book_metrics: &BookMetrics,
    trade_metrics: &TradeMetrics,
) -> Value {
    json!({
        "scenario": config.scenario_name,
        "order_count": config.synthetic_count,
        "best_bid": result.book.best_bid(),
        "best_ask": result.book.best_ask(),
        "resting_orders": result.book.resting_order_count(),
        "book_metrics": book_metrics_json(book_metrics),
        "trade_metrics": trade_metrics_json(trade_metrics),
    })
}

fn run_json(
    config: &CliConfig,
    result: &ScenarioResult,
    book_metrics: &BookMetrics,
    trade_metrics: &TradeMetrics,
) -> Value {
    json!({
        "scenario": config.scenario_name,
        "events": &result.events,
        "trades": &result.trades,
        "best_bid": result.book.best_bid(),
        "best_ask": result.book.best_ask(),
        "resting_orders": result.book.resting_order_count(),
        "snapshot": result.book.snapshot(),
        "book_metrics": book_metrics_json(book_metrics),
        "trade_metrics": trade_metrics_json(trade_metrics),
    })
}

fn book_metrics_json(metrics: &BookMetrics) -> Value {
    json!({
        "best_bid": metrics.best_bid,
        "best_ask": metrics.best_ask,
        "spread": metrics.spread,
        "mid_price": metrics.mid_price,
        "total_bid_quantity": metrics.total_bid_quantity,
        "total_ask_quantity": metrics.total_ask_quantity,
        "bid_price_levels": metrics.bid_price_levels,
        "ask_price_levels": metrics.ask_price_levels,
        "imbalance": metrics.imbalance,
    })
}

fn trade_metrics_json(metrics: &TradeMetrics) -> Value {
    json!({
        "trade_count": metrics.trade_count,
        "total_traded_quantity": metrics.total_traded_quantity,
        "total_notional": metrics.total_notional,
        "last_trade_price": metrics.last_trade_price,
        "vwap": metrics.vwap,
    })
}

fn write_json_file<T: serde::Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}
