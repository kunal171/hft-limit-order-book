use std::time::Duration;

use ::metrics::{Unit, describe_counter, describe_histogram};
use axum_prometheus::{
    AXUM_HTTP_REQUESTS_DURATION_SECONDS, PrometheusMetricLayer, PrometheusMetricLayerBuilder,
    metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle},
    utils::SECONDS_DURATION_BUCKETS,
};

pub const DATABASE_HEALTH_CHECKS_TOTAL: &str = "lob_database_health_checks_total";

pub const DATABASE_HEALTH_CHECK_DURATION_SECONDS: &str =
    "lob_database_health_check_duration_seconds";

// Buckets are measured in seconds.
// These cover 0.1 ms through 1 second.
const DATABASE_DURATION_BUCKETS: &[f64] = &[
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

pub fn initialize_prometheus() -> (PrometheusMetricLayer<'static>, PrometheusHandle) {
    // Configure histogram buckets before installing the global recorder.
    let recorder = PrometheusBuilder::new()
        // Keep the standard HTTP latency buckets.
        .set_buckets_for_metric(
            Matcher::Full(AXUM_HTTP_REQUESTS_DURATION_SECONDS.to_string()),
            SECONDS_DURATION_BUCKETS,
        )
        .expect("HTTP histogram buckets should be valid")
        // Configure finer buckets for the database health query.
        .set_buckets_for_metric(
            Matcher::Full(DATABASE_HEALTH_CHECK_DURATION_SECONDS.to_string()),
            DATABASE_DURATION_BUCKETS,
        )
        .expect("database histogram buckets should be valid")
        .build_recorder();

    let metric_handle = recorder.handle();

    // A process can have only one global metrics recorder.
    ::metrics::set_global_recorder(recorder)
        .expect("metrics recorder should only be installed once");

    let (prometheus_layer, _) = PrometheusMetricLayerBuilder::new()
        // Prometheus already monitors scrape success using its `up` metric.
        // Ignoring this route prevents scrapes from affecting API statistics.
        .with_ignore_pattern("/metrics")
        .with_metrics_from_fn(|| metric_handle.clone())
        .build_pair();

    // Add Prometheus HELP and TYPE metadata.
    describe_counter!(
        DATABASE_HEALTH_CHECKS_TOTAL,
        Unit::Count,
        "Total database health checks grouped by outcome."
    );

    describe_histogram!(
        DATABASE_HEALTH_CHECK_DURATION_SECONDS,
        Unit::Seconds,
        "Duration of the database health-check query."
    );

    // A manually built recorder requires periodic upkeep.
    let upkeep_handle = metric_handle.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            upkeep_handle.run_upkeep();
        }
    });

    (prometheus_layer, metric_handle)
}
