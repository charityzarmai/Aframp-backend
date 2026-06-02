//! SAR data models — full schema matching 20260528000000_sar_full_schema.sql

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SarStatus {
    Draft,
    UnderReview,
    Approved,
    Filed,
    Acknowledged,
    Rejected,
    ReturnedForRevision,
}

impl std::fmt::Display for SarStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Draft => "draft",
            Self::UnderReview => "under_review",
            Self::Approved => "approved",
            Self::Filed => "filed",
            Self::Acknowledged => "acknowledged",
            Self::Rejected => "rejected",
            Self::ReturnedForRevision => "returned_for_revision",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for SarStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "under_review" => Ok(Self::UnderReview),
            "approved" => Ok(Self::Approved),
            "filed" => Ok(Self::Filed),
            "acknowledged" => Ok(Self::Acknowledged),
            "rejected" => Ok(Self::Rejected),
            "returned_for_revision" => Ok(Self::ReturnedForRevision),
            other => Err(anyhow::anyhow!("unknown SAR status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SarType {
    TransactionBased,
    ActivityBased,
    ThresholdBased,
}

impl std::fmt::Display for SarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransactionBased => write!(f, "transaction_based"),
            Self::ActivityBased => write!(f, "activity_based"),
            Self::ThresholdBased => write!(f, "threshold_based"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    Individual,
    Entity,
}

impl std::fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Individual => write!(f, "individual"),
            Self::Entity => write!(f, "entity"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    AmlRuleTrigger,
    ComplianceOfficerJudgment,
    LawEnforcementRequest,
    SanctionsMatch,
}

impl std::fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmlRuleTrigger => write!(f, "aml_rule_trigger"),
            Self::ComplianceOfficerJudgment => write!(f, "compliance_officer_judgment"),
            Self::LawEnforcementRequest => write!(f, "law_enforcement_request"),
            Self::SanctionsMatch => write!(f, "sanctions_match"),
        }
    }
}

// ── Core SAR record ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarReport {
    pub id: Uuid,
    pub sar_type: String,
    pub status: String,
    pub subject_type: String,
    pub detection_method: String,
    pub subject_kyc_id: Option<Uuid>,
    pub subject_wallet_addresses: Vec<String>,
    pub suspicious_activity_description: String,
    pub activity_start_date: NaiveDate,
    pub activity_end_date: NaiveDate,
    pub total_amount_ngn: rust_decimal::Decimal,
    pub transaction_count: i32,
    pub linked_transaction_ids: Vec<Uuid>,
    pub aml_case_id: Option<Uuid>,
    pub aml_risk_score: Option<rust_decimal::Decimal>,
    pub triggered_rules: serde_json::Value,
    pub detecting_officer_id: Option<Uuid>,
    pub assigned_investigator_id: Option<Uuid>,
    pub reviewing_officer_id: Option<Uuid>,
    pub approving_officer_id: Option<Uuid>,
    pub investigation_checklist: serde_json::Value,
    pub filing_deadline: NaiveDate,
    pub filing_timestamp: Option<DateTime<Utc>>,
    pub filing_method: Option<String>,
    pub regulatory_reference_number: Option<String>,
    pub rejection_reason: Option<String>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledgement_reference: Option<String>,
    pub authority: String,
    pub generated_document: Option<String>,
    pub document_generated_at: Option<DateTime<Utc>>,
    pub retention_expires_at: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── SAR subject ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarSubject {
    pub id: Uuid,
    pub sar_id: Uuid,
    pub full_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub identification_docs: serde_json::Value,
    pub address: Option<String>,
    pub contact_info: serde_json::Value,
    pub platform_relationship: String,
    pub created_at: DateTime<Utc>,
}

// ── SAR transaction ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarTransaction {
    pub id: Uuid,
    pub sar_id: Uuid,
    pub transaction_id: Uuid,
    pub transaction_date: DateTime<Utc>,
    pub amount_ngn: rust_decimal::Decimal,
    pub transaction_type: String,
    pub counterparty_details: serde_json::Value,
    pub suspicious_element: String,
    pub created_at: DateTime<Utc>,
}

// ── SAR narrative (versioned) ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarNarrative {
    pub id: Uuid,
    pub sar_id: Uuid,
    pub version: i32,
    pub narrative_text: String,
    pub author_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ── SAR audit log ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarAuditEntry {
    pub id: Uuid,
    pub sar_id: Uuid,
    pub actor_id: String,
    pub action: String,
    pub from_status: String,
    pub to_status: String,
    pub notes: Option<String>,
    pub access_type: String,
    pub created_at: DateTime<Utc>,
}

// ── Request / Response DTOs ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSarRequest {
    pub sar_type: SarType,
    pub subject_type: SubjectType,
    pub detection_method: DetectionMethod,
    pub subject_kyc_id: Option<Uuid>,
    pub subject_wallet_addresses: Vec<String>,
    pub suspicious_activity_description: String,
    pub activity_start_date: NaiveDate,
    pub activity_end_date: NaiveDate,
    pub total_amount_ngn: rust_decimal::Decimal,
    pub transaction_count: i32,
    pub linked_transaction_ids: Vec<Uuid>,
    pub detecting_officer_id: Option<Uuid>,
    pub assigned_investigator_id: Option<Uuid>,
    /// Filing deadline days from today (defaults to SAR_FILING_DEADLINE_DAYS env var)
    pub deadline_days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddSubjectRequest {
    pub full_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub identification_docs: Option<serde_json::Value>,
    pub address: Option<String>,
    pub contact_info: Option<serde_json::Value>,
    pub platform_relationship: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTransactionRequest {
    pub transaction_id: Uuid,
    pub transaction_date: DateTime<Utc>,
    pub amount_ngn: rust_decimal::Decimal,
    pub transaction_type: String,
    pub counterparty_details: Option<serde_json::Value>,
    pub suspicious_element: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNarrativeRequest {
    pub narrative_text: String,
    pub author_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ReviewActionRequest {
    pub officer_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReturnForRevisionRequest {
    pub officer_id: Uuid,
    pub required_revisions: String,
}

#[derive(Debug, Deserialize)]
pub struct FileRequest {
    pub filing_method: String,
    pub regulatory_reference_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AcknowledgementRequest {
    pub acknowledgement_reference: String,
    pub officer_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct FilingRejectionRequest {
    pub rejection_reason: String,
    pub officer_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SarListQuery {
    pub status: Option<String>,
    pub subject_type: Option<String>,
    pub detection_method: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub from_date: DateTime<Utc>,
    pub to_date: DateTime<Utc>,
}

/// Full SAR detail response including related records
#[derive(Debug, Serialize)]
pub struct SarDetail {
    pub report: SarReport,
    pub subjects: Vec<SarSubject>,
    pub transactions: Vec<SarTransaction>,
    pub narratives: Vec<SarNarrative>,
    pub audit_log: Vec<SarAuditEntry>,
}

/// SAR filing metrics
#[derive(Debug, Serialize)]
pub struct SarMetrics {
    pub period_from: DateTime<Utc>,
    pub period_to: DateTime<Utc>,
    pub total_initiated: i64,
    pub total_filed: i64,
    pub total_rejected_by_regulator: i64,
    pub total_overdue: i64,
    pub avg_days_detection_to_filing: f64,
    pub filing_timeliness_rate: f64,
    pub by_detection_method: serde_json::Value,
    pub by_subject_type: serde_json::Value,
}

/// Deadline status entry
#[derive(Debug, Serialize)]
pub struct SarDeadlineStatus {
    pub sar_id: Uuid,
    pub status: String,
    pub filing_deadline: NaiveDate,
    pub days_remaining: i64,
    pub assigned_investigator_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Investigation checklist — all fields must be true before submit-for-review
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InvestigationChecklist {
    pub subject_identity_verified: bool,
    pub transaction_records_reviewed: bool,
    pub aml_rules_documented: bool,
    pub narrative_complete: bool,
    pub supporting_docs_attached: bool,
    pub legal_review_complete: bool,
}

impl InvestigationChecklist {
    pub fn is_complete(&self) -> bool {
        self.subject_identity_verified
            && self.transaction_records_reviewed
            && self.aml_rules_documented
            && self.narrative_complete
            && self.supporting_docs_attached
            && self.legal_review_complete
    }
}
