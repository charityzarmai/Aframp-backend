//! SAR Prometheus metrics

use prometheus::{register_counter_vec, register_gauge_vec, CounterVec, GaugeVec};
use std::sync::OnceLock;

static SAR_INITIATED: OnceLock<CounterVec> = OnceLock::new();
static SAR_FILED: OnceLock<CounterVec> = OnceLock::new();
static SAR_REJECTED_BY_REGULATOR: OnceLock<CounterVec> = OnceLock::new();
static SAR_PAST_DEADLINE: OnceLock<CounterVec> = OnceLock::new();
static SAR_OPEN_BY_STATUS: OnceLock<GaugeVec> = OnceLock::new();
static SAR_DAYS_UNTIL_NEAREST_DEADLINE: OnceLock<GaugeVec> = OnceLock::new();
static SAR_OVERDUE_COUNT: OnceLock<GaugeVec> = OnceLock::new();

pub fn inc_initiated(detection_method: &str) {
    SAR_INITIATED
        .get_or_init(|| {
            register_counter_vec!(
                "aframp_sar_initiated_total",
                "SARs initiated by detection method",
                &["detection_method"]
            )
            .expect("register sar_initiated")
        })
        .with_label_values(&[detection_method])
        .inc();
}

pub fn inc_filed(filing_method: &str) {
    SAR_FILED
        .get_or_init(|| {
            register_counter_vec!(
                "aframp_sar_filed_total",
                "SARs filed",
                &["filing_method"]
            )
            .expect("register sar_filed")
        })
        .with_label_values(&[filing_method])
        .inc();
}

pub fn inc_rejected_by_regulator(authority: &str) {
    SAR_REJECTED_BY_REGULATOR
        .get_or_init(|| {
            register_counter_vec!(
                "aframp_sar_rejected_by_regulator_total",
                "SARs rejected by regulator",
                &["authority"]
            )
            .expect("register sar_rejected_by_regulator")
        })
        .with_label_values(&[authority])
        .inc();
}

pub fn inc_past_deadline(detection_method: &str) {
    SAR_PAST_DEADLINE
        .get_or_init(|| {
            register_counter_vec!(
                "aframp_sar_past_deadline_total",
                "SARs that reached deadline without filing",
                &["detection_method"]
            )
            .expect("register sar_past_deadline")
        })
        .with_label_values(&[detection_method])
        .inc();
}

pub fn set_open_by_status(status: &str, count: f64) {
    SAR_OPEN_BY_STATUS
        .get_or_init(|| {
            register_gauge_vec!(
                "aframp_sar_open_count",
                "Open SAR count per status",
                &["status"]
            )
            .expect("register sar_open_by_status")
        })
        .with_label_values(&[status])
        .set(count);
}

pub fn set_days_until_nearest_deadline(days: f64) {
    SAR_DAYS_UNTIL_NEAREST_DEADLINE
        .get_or_init(|| {
            register_gauge_vec!(
                "aframp_sar_days_until_nearest_deadline",
                "Days until nearest SAR deadline",
                &[]
            )
            .expect("register sar_days_until_nearest_deadline")
        })
        .with_label_values(&[])
        .set(days);
}

pub fn set_overdue_count(count: f64) {
    SAR_OVERDUE_COUNT
        .get_or_init(|| {
            register_gauge_vec!(
                "aframp_sar_overdue_count",
                "SARs past their filing deadline",
                &[]
            )
            .expect("register sar_overdue_count")
        })
        .with_label_values(&[])
        .set(count);
}
