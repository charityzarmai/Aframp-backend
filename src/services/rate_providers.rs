//! Rate providers for fetching exchange rates
//!
//! Implements different rate providers:
//! - FixedRateProvider: For cNGN 1:1 peg with NGN
//! - ExternalApiProvider: For future external API integration

use super::exchange_rate::{ExchangeRateError, ExchangeRateResult, RateData, RateProvider};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::Utc;
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::str::FromStr;
use std::time::Duration;
use tracing::debug;

/// Fixed rate provider for cNGN/NGN 1:1 peg
pub struct FixedRateProvider {
    supported_pairs: Vec<(String, String)>,
}

impl FixedRateProvider {
    pub fn new() -> Self {
        Self {
            supported_pairs: vec![
                ("NGN".to_string(), "cNGN".to_string()),
                ("cNGN".to_string(), "NGN".to_string()),
            ],
        }
    }
}

impl Default for FixedRateProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RateProvider for FixedRateProvider {
    async fn fetch_rate(&self, from: &str, to: &str) -> ExchangeRateResult<RateData> {
        // Check if this is a supported pair
        let is_supported = self
            .supported_pairs
            .iter()
            .any(|(f, t)| f == from && t == to);

        if !is_supported {
            return Err(ExchangeRateError::RateNotFound {
                from: from.to_string(),
                to: to.to_string(),
            });
        }

        // Return fixed 1:1 rate
        let one = BigDecimal::from(1);

        Ok(RateData {
            currency_pair: format!("{}/{}", from, to),
            base_rate: one.clone(),
            buy_rate: one.clone(),
            sell_rate: one.clone(),
            spread: BigDecimal::from(0),
            source: "fixed_peg".to_string(),
            last_updated: Utc::now(),
        })
    }

    fn get_supported_pairs(&self) -> Vec<(String, String)> {
        self.supported_pairs.clone()
    }

    async fn is_healthy(&self) -> bool {
        true // Always healthy since it's a fixed rate
    }

    fn name(&self) -> &str {
        "FixedRateProvider"
    }
}

/// External API provider for fetching rates from external sources
/// This is a placeholder for future implementation with CoinGecko, Fixer.io, etc.
pub struct ExternalApiProvider {
    api_url: String,
    api_key: Option<String>,
    supported_pairs: Vec<(String, String)>,
    timeout_seconds: u64,
    http_client: HttpClient,
}

impl ExternalApiProvider {
    pub fn new(api_url: String, api_key: Option<String>) -> Self {
        Self {
            api_url,
            api_key,
            supported_pairs: Vec::new(),
            timeout_seconds: 10,
            http_client: HttpClient::new(),
        }
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn add_supported_pair(mut self, from: String, to: String) -> Self {
        self.supported_pairs.push((from, to));
        self
    }

    fn parse_decimal(value: &Value) -> Option<BigDecimal> {
        match value {
            Value::Number(num) => BigDecimal::from_str(&num.to_string()).ok(),
            Value::String(s) => BigDecimal::from_str(s).ok(),
            _ => None,
        }
    }

    fn extract_rate(payload: &Value, to: &str) -> Option<BigDecimal> {
        let direct_rate_candidates = [
            payload.get("rate"),
            payload.get("result"),
            payload.get("price"),
            payload.get("conversion_rate"),
            payload.get("buy_rate"),
            payload.get("sell_rate"),
            payload.get("data").and_then(|v| v.get("rate")),
            payload.get("data").and_then(|v| v.get("result")),
            payload.get("data").and_then(|v| v.get("price")),
            payload.get("info").and_then(|v| v.get("rate")),
        ];

        for candidate in direct_rate_candidates.into_iter().flatten() {
            if let Some(rate) = Self::parse_decimal(candidate) {
                return Some(rate);
            }
        }

        let to_upper = to.to_uppercase();
        let to_lower = to.to_lowercase();
        let rates_objects = [
            payload.get("rates"),
            payload.get("result"),
            payload.get("data").and_then(|v| v.get("rates")),
            payload.get("data").and_then(|v| v.get("result")),
        ];

        for maybe_rates in rates_objects {
            if let Some(rates) = maybe_rates.and_then(|v| v.as_object()) {
                for key in [to, to_upper.as_str(), to_lower.as_str()] {
                    if let Some(value) = rates.get(key) {
                        if let Some(rate) = Self::parse_decimal(value) {
                            return Some(rate);
                        }
                    }
                }
            }
        }

        None
    }

    async fn request_rate(&self, from: &str, to: &str) -> ExchangeRateResult<BigDecimal> {
        let is_fastforex = self.api_url.contains("fastforex.io");
        let mut url = reqwest::Url::parse(&self.api_url).map_err(|e| {
            ExchangeRateError::ProviderError(format!("Invalid external API URL: {}", e))
        })?;
        let has_api_key = url.query_pairs().any(|(k, _)| k == "api_key");
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("from", from);
            query.append_pair("to", to);
            if is_fastforex {
                if let Some(api_key) = &self.api_key {
                    if !has_api_key {
                        query.append_pair("api_key", api_key);
                    }
                }
            }
        }

        let mut request = self
            .http_client
            .get(url)
            .timeout(Duration::from_secs(self.timeout_seconds));

        if let Some(api_key) = &self.api_key {
            request = request.header("x-api-key", api_key);
            if !is_fastforex {
                request = request.bearer_auth(api_key);
            }
        }

        let response = request.send().await.map_err(|e| {
            ExchangeRateError::ProviderError(format!("External API request failed: {}", e))
        })?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let body_snippet: String = body.chars().take(256).collect();
            let message = if body_snippet.is_empty() {
                format!("External API returned error status {}", status)
            } else {
                format!(
                    "External API returned error status {}: {}",
                    status, body_snippet
                )
            };
            return Err(ExchangeRateError::ProviderError(message));
        }

        let payload: Value = response.json().await.map_err(|e| {
            ExchangeRateError::ProviderError(format!("Invalid external API JSON response: {}", e))
        })?;

        let rate = Self::extract_rate(&payload, to).ok_or_else(|| {
            ExchangeRateError::ProviderError(format!(
                "External API response does not contain a usable rate for {}/{}",
                from, to
            ))
        })?;

        if rate <= BigDecimal::from(0) {
            return Err(ExchangeRateError::InvalidRate(format!(
                "External API returned non-positive rate for {}/{}",
                from, to
            )));
        }

        Ok(rate)
    }
}

#[async_trait]
impl RateProvider for ExternalApiProvider {
    async fn fetch_rate(&self, from: &str, to: &str) -> ExchangeRateResult<RateData> {
        // Check if this is a supported pair
        let is_supported = self.supported_pairs.is_empty()
            || self
                .supported_pairs
                .iter()
                .any(|(f, t)| f == from && t == to);

        if !is_supported {
            return Err(ExchangeRateError::RateNotFound {
                from: from.to_string(),
                to: to.to_string(),
            });
        }

        debug!(
            "ExternalApiProvider: Fetching rate from {} for {} -> {}",
            self.api_url, from, to
        );

        let rate = match self.request_rate(from, to).await {
            Ok(rate) => rate,
            Err(direct_error) => match self.request_rate(to, from).await {
                Ok(reverse_rate) => BigDecimal::from(1) / reverse_rate,
                Err(reverse_error) => {
                    return Err(ExchangeRateError::ProviderError(format!(
                        "Failed to fetch direct {}/{} ({}) and reverse {}/{} ({})",
                        from, to, direct_error, to, from, reverse_error
                    )));
                }
            },
        };

        Ok(RateData {
            currency_pair: format!("{}/{}", from, to),
            base_rate: rate.clone(),
            buy_rate: rate.clone(),
            sell_rate: rate,
            spread: BigDecimal::from(0),
            source: self.api_url.clone(),
            last_updated: Utc::now(),
        })
    }

    fn get_supported_pairs(&self) -> Vec<(String, String)> {
        self.supported_pairs.clone()
    }

    async fn is_healthy(&self) -> bool {
        let (from, to) = self
            .supported_pairs
            .first()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .unwrap_or(("NGN", "USD"));

        self.request_rate(from, to).await.is_ok() || self.request_rate(to, from).await.is_ok()
    }

    fn name(&self) -> &str {
        "ExternalApiProvider"
    }
}

/// Multi-source rate provider that aggregates rates from multiple sources
pub struct AggregatedRateProvider {
    providers: Vec<Box<dyn RateProvider>>,
    aggregation_strategy: AggregationStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum AggregationStrategy {
    Average,
    Median,
    First,
}

impl AggregatedRateProvider {
    pub fn new(strategy: AggregationStrategy) -> Self {
        Self {
            providers: Vec::new(),
            aggregation_strategy: strategy,
        }
    }

    pub fn add_provider(mut self, provider: Box<dyn RateProvider>) -> Self {
        self.providers.push(provider);
        self
    }
}

#[async_trait]
impl RateProvider for AggregatedRateProvider {
    async fn fetch_rate(&self, from: &str, to: &str) -> ExchangeRateResult<RateData> {
        if self.providers.is_empty() {
            return Err(ExchangeRateError::ProviderError(
                "No providers configured".to_string(),
            ));
        }

        let mut rates = Vec::new();
        let mut last_error = None;

        // Fetch rates from all providers
        for provider in &self.providers {
            if provider.is_healthy().await {
                match provider.fetch_rate(from, to).await {
                    Ok(rate_data) => rates.push(rate_data),
                    Err(e) => {
                        last_error = Some(e);
                        continue;
                    }
                }
            }
        }

        if rates.is_empty() {
            return Err(last_error.unwrap_or_else(|| {
                ExchangeRateError::ProviderError("All providers failed".to_string())
            }));
        }

        // Aggregate rates based on strategy
        let aggregated_rate = match self.aggregation_strategy {
            AggregationStrategy::First => rates[0].base_rate.clone(),
            AggregationStrategy::Average => {
                let sum: BigDecimal = rates.iter().map(|r| &r.base_rate).sum();
                sum / BigDecimal::from(rates.len() as u64)
            }
            AggregationStrategy::Median => {
                let mut sorted_rates: Vec<BigDecimal> =
                    rates.iter().map(|r| r.base_rate.clone()).collect();
                sorted_rates.sort();
                let mid = sorted_rates.len() / 2;
                if sorted_rates.len() % 2 == 0 {
                    (&sorted_rates[mid - 1] + &sorted_rates[mid]) / BigDecimal::from(2)
                } else {
                    sorted_rates[mid].clone()
                }
            }
        };

        Ok(RateData {
            currency_pair: format!("{}/{}", from, to),
            base_rate: aggregated_rate.clone(),
            buy_rate: aggregated_rate.clone(),
            sell_rate: aggregated_rate.clone(),
            spread: BigDecimal::from(0),
            source: format!("aggregated_{:?}", self.aggregation_strategy),
            last_updated: Utc::now(),
        })
    }

    fn get_supported_pairs(&self) -> Vec<(String, String)> {
        // Return union of all supported pairs
        let mut pairs = Vec::new();
        for provider in &self.providers {
            for pair in provider.get_supported_pairs() {
                if !pairs.contains(&pair) {
                    pairs.push(pair);
                }
            }
        }
        pairs
    }

    async fn is_healthy(&self) -> bool {
        // At least one provider must be healthy
        for provider in &self.providers {
            if provider.is_healthy().await {
                return true;
            }
        }
        false
    }

    fn name(&self) -> &str {
        "AggregatedRateProvider"
    }
}

/// Mock rate provider for testing
#[cfg(test)]
pub struct MockRateProvider {
    rate: BigDecimal,
    healthy: bool,
}

#[cfg(test)]
impl MockRateProvider {
    pub fn new(rate: f64) -> Self {
        Self {
            rate: BigDecimal::from_str(&rate.to_string()).unwrap(),
            healthy: true,
        }
    }

    pub fn with_health(mut self, healthy: bool) -> Self {
        self.healthy = healthy;
        self
    }
}

#[cfg(test)]
#[async_trait]
impl RateProvider for MockRateProvider {
    async fn fetch_rate(&self, from: &str, to: &str) -> ExchangeRateResult<RateData> {
        Ok(RateData {
            currency_pair: format!("{}/{}", from, to),
            base_rate: self.rate.clone(),
            buy_rate: self.rate.clone(),
            sell_rate: self.rate.clone(),
            spread: BigDecimal::from(0),
            source: "mock".to_string(),
            last_updated: Utc::now(),
        })
    }

    fn get_supported_pairs(&self) -> Vec<(String, String)> {
        vec![("USD".to_string(), "NGN".to_string())]
    }

    async fn is_healthy(&self) -> bool {
        self.healthy
    }

    fn name(&self) -> &str {
        "MockRateProvider"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fixed_rate_provider() {
        let provider = FixedRateProvider::new();

        // Test NGN -> cNGN
        let rate = provider.fetch_rate("NGN", "cNGN").await.unwrap();
        assert_eq!(rate.base_rate, BigDecimal::from(1));
        assert_eq!(rate.source, "fixed_peg");

        // Test cNGN -> NGN
        let rate = provider.fetch_rate("cNGN", "NGN").await.unwrap();
        assert_eq!(rate.base_rate, BigDecimal::from(1));

        // Test unsupported pair
        let result = provider.fetch_rate("USD", "NGN").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fixed_rate_provider_health() {
        let provider = FixedRateProvider::new();
        assert!(provider.is_healthy().await);
    }

    #[tokio::test]
    async fn test_aggregated_provider_average() {
        let provider1 = Box::new(MockRateProvider::new(1500.0));
        let provider2 = Box::new(MockRateProvider::new(1600.0));

        let aggregated = AggregatedRateProvider::new(AggregationStrategy::Average)
            .add_provider(provider1)
            .add_provider(provider2);

        let rate = aggregated.fetch_rate("USD", "NGN").await.unwrap();
        let expected = BigDecimal::from_str("1550").unwrap();
        assert_eq!(rate.base_rate, expected);
    }

    #[tokio::test]
    async fn test_aggregated_provider_median() {
        let provider1 = Box::new(MockRateProvider::new(1500.0));
        let provider2 = Box::new(MockRateProvider::new(1600.0));
        let provider3 = Box::new(MockRateProvider::new(1700.0));

        let aggregated = AggregatedRateProvider::new(AggregationStrategy::Median)
            .add_provider(provider1)
            .add_provider(provider2)
            .add_provider(provider3);

        let rate = aggregated.fetch_rate("USD", "NGN").await.unwrap();
        let expected = BigDecimal::from_str("1600").unwrap();
        assert_eq!(rate.base_rate, expected);
    }
}
