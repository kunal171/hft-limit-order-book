use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use limit_order_book::simulator::{
    GeneratorConfig, ScenarioCommand, generate_crossing_orders, generate_two_sided_orders,
    run_scenario,
};
use limit_order_book::{EventMode, Order, OrderBook, OrderBookConfig, Side};
use std::hint::black_box;

/// Helper Functions
fn build_deep_book(order_count: u64) -> OrderBook {
    let mut book = OrderBook::new();

    for id in 1..=order_count {
        // Same price level creates a deep FIFO queue.
        // This is intentionally bad for current cancel scanning.
        book.add_order(Order::new(id, Side::Buy, 100, 10))
            .expect("setup order should be accepted");
    }

    book
}

fn build_deep_book_with_stale_ids(order_count: u64) -> OrderBook {
    let mut book = build_deep_book(order_count);

    for id in 1..order_count {
        // Lazy cancel leaves stale IDs in the FIFO queue, but removes active
        // liquidity from the price level's total quantity.
        book.cancel_order(id)
            .expect("setup cancel should remove active order");
    }

    book
}

fn build_book_with_cancelled_price_levels(price_level_count: u64) -> OrderBook {
    let mut book = OrderBook::new();

    for id in 1..=price_level_count {
        book.add_order(Order::new(id, Side::Buy, id, 10))
            .expect("setup order should be accepted");
    }

    for id in 2..=price_level_count {
        // Leaves the lowest price active and cancels every better bid level.
        // The benchmark verifies best_bid does not pay to scan dead levels.
        book.cancel_order(id)
            .expect("setup cancel should remove active liquidity");
    }

    book
}

fn run_commands_with_event_mode(commands: &[ScenarioCommand], event_mode: EventMode) -> OrderBook {
    let mut book = OrderBook::with_config(OrderBookConfig { event_mode });

    for command in commands {
        match command {
            ScenarioCommand::Add(order) => {
                book.add_order(order.clone())
                    .expect("order should be accepted");
            }
            ScenarioCommand::Cancel { order_id } => {
                book.cancel_order(*order_id).expect("cancel should succeed");
            }
            ScenarioCommand::Modify {
                order_id,
                new_price,
                new_quantity,
            } => {
                book.modify_order(*order_id, *new_price, *new_quantity)
                    .expect("modify should succeed");
            }
        }
    }

    book
}

fn bench_two_sided_synthetic(c: &mut Criterion) {
    // Pre-generate commands so we measure order book execution,
    // not synthetic data creation.
    let commands = generate_two_sided_orders(GeneratorConfig {
        order_count: 1_000,
        start_order_id: 1,
        base_price: 100,
        tick_size: 1,
        price_levels: 10,
        quantity: 5,
    });

    c.bench_function("two_sided_1000_orders", |b| {
        b.iter(|| {
            run_scenario(black_box(&commands)).expect("benchmark scenario should run");
        });
    });
}

fn bench_crossing_synthetic(c: &mut Criterion) {
    // This workload creates trades, so it measures matching pressure.
    let commands = generate_crossing_orders(GeneratorConfig {
        order_count: 1_000,
        start_order_id: 1,
        base_price: 100,
        tick_size: 1,
        price_levels: 10,
        quantity: 5,
    });

    c.bench_function("crossing_1000_orders", |b| {
        b.iter(|| {
            run_scenario(black_box(&commands)).expect("benchmark scenario should run");
        });
    });
}

fn bench_hot_path_operations(c: &mut Criterion) {
    c.bench_function("add_one_resting_order", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();

            book.add_order(black_box(Order::new(1, Side::Buy, 100, 10)))
                .expect("order should be accepted");
        });
    });

    c.bench_function("single_trade_ref", |b| {
        b.iter_batched_ref(
            || {
                let mut book = OrderBook::new();

                // Setup resting liquidity outside the measured closure.
                book.add_order(Order::new(1, Side::Buy, 100, 10))
                    .expect("setup order should be accepted");

                book
            },
            |book| {
                // Measures only the crossing sell order.
                book.add_order(black_box(Order::new(2, Side::Sell, 100, 5)))
                    .expect("crossing order should be accepted");
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("multi_level_sweep_ref", |b| {
        b.iter_batched_ref(
            || {
                let mut book = OrderBook::new();

                // Setup multiple ask levels outside the measured closure.
                book.add_order(Order::new(1, Side::Sell, 100, 5)).unwrap();
                book.add_order(Order::new(2, Side::Sell, 101, 5)).unwrap();
                book.add_order(Order::new(3, Side::Sell, 102, 5)).unwrap();

                book
            },
            |book| {
                // Measures one buy order sweeping three ask levels.
                book.add_order(black_box(Order::new(4, Side::Buy, 102, 12)))
                    .expect("sweep order should be accepted");
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("cancel_order_ref", |b| {
        b.iter_batched_ref(
            || {
                let mut book = OrderBook::new();

                // Setup one resting order outside the measured closure.
                book.add_order(Order::new(1, Side::Buy, 100, 10))
                    .expect("setup order should be accepted");

                book
            },
            |book| {
                // Measures removing an active resting order.
                book.cancel_order(black_box(1))
                    .expect("cancel should succeed");
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("modify_order_ref", |b| {
        b.iter_batched_ref(
            || {
                let mut book = OrderBook::new();

                // Setup one resting order outside the measured closure.
                book.add_order(Order::new(1, Side::Buy, 100, 10))
                    .expect("setup order should be accepted");

                book
            },
            |book| {
                // Measures changing price and quantity of an active order.
                book.modify_order(black_box(1), black_box(101), black_box(7))
                    .expect("modify should succeed");
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_large_two_sided_books(c: &mut Criterion) {
    // Test bigger resting books before optimizing.
    // This shows how the current BTreeMap + VecDeque structure scales.

    for order_count in [10_000, 100_000] {
        let commands = generate_two_sided_orders(GeneratorConfig {
            order_count,
            start_order_id: 1,
            base_price: 100_000,
            tick_size: 1,
            price_levels: 100,
            quantity: 5,
        });

        c.bench_function(&format!("two_sided_{order_count}_orders"), |b| {
            b.iter(|| {
                run_scenario(black_box(&commands)).expect("benchmark scenario should run");
            });
        });
    }
}

fn bench_deep_cancel_modify(c: &mut Criterion) {
    c.bench_function("cancel_from_10000_deep_level_ref", |b| {
        b.iter_batched_ref(
            || build_deep_book(10_000),
            |book| {
                // Cancel near the end to expose scan cost.
                book.cancel_order(black_box(9_999))
                    .expect("cancel should succeed");
            },
            BatchSize::LargeInput,
        );
    });

    c.bench_function("cancel_after_9999_lazy_cancels_ref", |b| {
        b.iter_batched_ref(
            || build_deep_book_with_stale_ids(10_000),
            |book| {
                // This should stay fast even though the FIFO queue contains
                // many stale IDs before this active order.
                book.cancel_order(black_box(10_000))
                    .expect("last active cancel should succeed");
            },
            BatchSize::LargeInput,
        );
    });

    c.bench_function("modify_from_10000_deep_level_ref", |b| {
        b.iter_batched_ref(
            || build_deep_book(10_000),
            |book| {
                // Modify also removes first, so it exposes the same lookup weakness.
                book.modify_order(black_box(9_999), black_box(101), black_box(7))
                    .expect("modify should succeed");
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_price_level_liquidity_queries(c: &mut Criterion) {
    let active_book = build_deep_book(10_000);
    let stale_book = build_deep_book_with_stale_ids(10_000);
    let cancelled_levels_book = build_book_with_cancelled_price_levels(10_000);

    c.bench_function("best_bid_from_price_level_quantity_10000_orders", |b| {
        b.iter(|| {
            black_box(active_book.best_bid());
        });
    });

    c.bench_function("resting_count_from_10000_active_orders", |b| {
        b.iter(|| {
            black_box(active_book.resting_order_count());
        });
    });

    c.bench_function("best_bid_after_9999_same_level_lazy_cancels", |b| {
        b.iter(|| {
            black_box(stale_book.best_bid());
        });
    });

    c.bench_function("best_bid_after_9999_cancelled_price_levels", |b| {
        b.iter(|| {
            black_box(cancelled_levels_book.best_bid());
        });
    });
}

fn bench_multi_symbol_workload(c: &mut Criterion) {
    let symbol_count = 100;
    let orders_per_symbol = 1_000;

    let commands = generate_two_sided_orders(GeneratorConfig {
        order_count: orders_per_symbol,
        start_order_id: 1,
        base_price: 100_000,
        tick_size: 1,
        price_levels: 50,
        quantity: 5,
    });

    c.bench_function("multi_symbol_100x1000_orders", |b| {
        b.iter(|| {
            let mut books = Vec::with_capacity(symbol_count);

            for _ in 0..symbol_count {
                // Each OrderBook represents one instrument/symbol.
                let result =
                    run_scenario(black_box(&commands)).expect("symbol scenario should run");
                books.push(result.book);
            }

            black_box(books);
        });
    });
}

fn bench_event_modes(c: &mut Criterion) {
    let commands = generate_crossing_orders(GeneratorConfig {
        order_count: 1_000,
        start_order_id: 1,
        base_price: 100,
        tick_size: 1,
        price_levels: 10,
        quantity: 5,
    });

    c.bench_function("crossing_1000_events_full", |b| {
        b.iter(|| {
            black_box(run_commands_with_event_mode(
                black_box(&commands),
                EventMode::Full,
            ));
        });
    });

    c.bench_function("crossing_1000_events_trades_only", |b| {
        b.iter(|| {
            black_box(run_commands_with_event_mode(
                black_box(&commands),
                EventMode::TradesOnly,
            ));
        });
    });

    c.bench_function("crossing_1000_events_disabled", |b| {
        b.iter(|| {
            black_box(run_commands_with_event_mode(
                black_box(&commands),
                EventMode::Disabled,
            ));
        });
    });
}

criterion_group!(
    benches,
    bench_two_sided_synthetic,
    bench_crossing_synthetic,
    bench_hot_path_operations,
    bench_large_two_sided_books,
    bench_deep_cancel_modify,
    bench_price_level_liquidity_queries,
    bench_multi_symbol_workload,
    bench_event_modes
);
criterion_main!(benches);
