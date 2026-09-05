//! src/engine/config.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMode {
    // Current behavior. Store all events so replay/debugging works.
    Full,

    // Store only trades. Useful when we care about executions but not replay.
    TradesOnly,

    // Store no events. Useful for hot-path latency benchmarks.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderBookConfig {
    pub event_mode: EventMode,
}

impl Default for OrderBookConfig {
    fn default() -> Self {
        Self {
            event_mode: EventMode::Full,
        }
    }
}
