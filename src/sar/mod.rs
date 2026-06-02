//! SAR (Suspicious Activity Report) management module
//!
//! State machine: draft → under_review → approved → filed → acknowledged
//!                                     ↘ returned_for_revision ↗
//!                                     ↘ rejected
//!
//! CONFIDENTIALITY: All SAR data is restricted to compliance officers.
//! No SAR data appears in standard application logs.
//! Tipping-off prevention: no subject-facing notifications are ever sent.

pub mod deadline_worker;
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod template;

#[cfg(test)]
pub mod tests;

pub use handlers::SarState;
pub use models::{
    CreateSarRequest, DetectionMethod, InvestigationChecklist, SarDetail, SarListQuery,
    SarMetrics, SarReport, SarStatus, SarType, SubjectType,
};
pub use routes::sar_routes;
pub use service::SarService;
