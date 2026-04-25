/// DeFi Integration Architecture & Protocol Selection (Issue #370)
///
/// This module provides the foundational DeFi integration architecture and protocol
/// selection framework that governs how the platform interacts with decentralized
/// finance protocols to generate yield on cNGN holdings and provide additional
/// financial products to users.

pub mod adapters;
pub mod amm;
pub mod evaluation;
pub mod governance;
pub mod handlers;
pub mod models;
pub mod protocols;
pub mod repository;
pub mod risk_controls;
pub mod savings;
pub mod service;
pub mod treasury;
pub mod types;

#[cfg(test)]
mod tests;

// ── Public exports ─────────────────────────────────────────────────────────────

pub use models::*;
pub use protocols::DeFiProtocol;
pub use risk_controls::{RiskController, CircuitBreaker};
pub use evaluation::{ProtocolEvaluator, EvaluationCriteria, RiskTier};
pub use governance::{GovernanceCommittee, ApprovalWorkflow};
pub use treasury::TreasuryManager;

// ── Module-level constants (all configurable via env at startup) ──────────────

/// Maximum percentage of platform treasury that may be deployed in DeFi protocols
pub const MAX_DEFI_TREASURY_EXPOSURE_PCT: f64 = 30.0;

/// Maximum percentage of funds that may be deployed in any single DeFi protocol
pub const MAX_SINGLE_PROTOCOL_EXPOSURE_PCT: f64 = 10.0;

/// Maximum amount for any single DeFi transaction (in cNGN units)
pub const MAX_SINGLE_TRANSACTION_AMOUNT: u64 = 1_000_000; // 1M cNGN

/// Default slippage tolerance for DeFi operations (1%)
pub const DEFAULT_SLIPPAGE_TOLERANCE: f64 = 0.01;

/// Circuit breaker TVL drop threshold (20% drop triggers withdrawal)
pub const CIRCUIT_BREAKER_TVL_DROP_THRESHOLD: f64 = 0.20;

/// Circuit breaker TVL drop window (24 hours)
pub const CIRCUIT_BREAKER_TVL_DROP_WINDOW_HOURS: i64 = 24;

/// Minimum number of governance committee approvals required for strategy activation
pub const MIN_GOVERNANCE_APPROVALS: usize = 3;

/// Background job intervals (in seconds)
pub const PROTOCOL_HEALTH_CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes
pub const TREASURY_EXPOSURE_CHECK_INTERVAL_SECS: u64 = 600; // 10 minutes
pub const YIELD_RATE_UPDATE_INTERVAL_SECS: u64 = 3600; // 1 hour
