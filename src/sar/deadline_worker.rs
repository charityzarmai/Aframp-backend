//! SAR deadline worker — runs on a configurable interval to check deadlines,
//! send reminders, and fire alerts for overdue SARs.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::error;

use super::service::SarService;

pub struct SarDeadlineWorker {
    svc: Arc<SarService>,
    interval: Duration,
}

impl SarDeadlineWorker {
    pub fn new(svc: Arc<SarService>) -> Self {
        let interval_secs = std::env::var("SAR_DEADLINE_CHECK_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600u64); // default: hourly
        Self { svc, interval: Duration::from_secs(interval_secs) }
    }

    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut ticker = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.svc.run_deadline_checks().await {
                        error!(error = %e, "SAR deadline check failed");
                    }
                }
                _ = shutdown.changed() => break,
            }
        }
    }
}
