//! SAR HTTP handlers — all routes under /api/admin/compliance/sars
//!
//! CONFIDENTIALITY: actor identity is extracted from CallerIdentity (set by RBAC middleware).
//! No SAR data is returned in error messages that could reach the subject.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::middleware::rbac::CallerIdentity;

use super::{models::*, service::SarService};

pub type SarState = Arc<SarService>;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn err(e: anyhow::Error) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
}

fn not_found() -> axum::response::Response {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "not_found" }))).into_response()
}

fn bad_request(msg: &str) -> axum::response::Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response()
}

// ── Initiation ───────────────────────────────────────────────────────────────

/// POST /api/admin/compliance/sars
pub async fn create_sar(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Json(body): Json<CreateSarRequest>,
) -> impl IntoResponse {
    let Ok(officer_id) = caller.user_id.parse::<Uuid>() else {
        return bad_request("officer_id in token is not a valid UUID");
    };
    match svc.manual_initiate(body, officer_id).await {
        Ok(r) => (StatusCode::CREATED, Json(serde_json::json!({ "sar": r }))).into_response(),
        Err(e) => err(e),
    }
}

// ── Investigation workflow ────────────────────────────────────────────────────

/// GET /api/admin/compliance/sars
pub async fn list_sars(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Query(q): Query<SarListQuery>,
) -> impl IntoResponse {
    match svc.list(&q, &caller.user_id).await {
        Ok(reports) => (StatusCode::OK, Json(serde_json::json!({ "sars": reports }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/admin/compliance/sars/:sar_id
pub async fn get_sar(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_detail(sar_id, &caller.user_id).await {
        Ok(Some(detail)) => (StatusCode::OK, Json(detail)).into_response(),
        Ok(None) => not_found(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/transactions
pub async fn add_transaction(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<AddTransactionRequest>,
) -> impl IntoResponse {
    match svc.add_transaction(sar_id, body, &caller.user_id).await {
        Ok(t) => (StatusCode::CREATED, Json(t)).into_response(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/subjects
pub async fn add_subject(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<AddSubjectRequest>,
) -> impl IntoResponse {
    match svc.add_subject(sar_id, body, &caller.user_id).await {
        Ok(s) => (StatusCode::CREATED, Json(s)).into_response(),
        Err(e) => err(e),
    }
}

/// PATCH /api/admin/compliance/sars/:sar_id/narrative
pub async fn update_narrative(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<UpdateNarrativeRequest>,
) -> impl IntoResponse {
    match svc.update_narrative(sar_id, body).await {
        Ok(n) => (StatusCode::OK, Json(n)).into_response(),
        Err(e) => err(e),
    }
}

/// PATCH /api/admin/compliance/sars/:sar_id/checklist
pub async fn update_checklist(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<InvestigationChecklist>,
) -> impl IntoResponse {
    match svc.update_checklist(sar_id, body, &caller.user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/submit-for-review
pub async fn submit_for_review(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.submit_for_review(sar_id, &caller.user_id).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) if e.to_string().contains("checklist") => bad_request(&e.to_string()),
        Err(e) => err(e),
    }
}

// ── Review / approval ─────────────────────────────────────────────────────────

/// POST /api/admin/compliance/sars/:sar_id/approve
pub async fn approve_sar(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<ReviewActionRequest>,
) -> impl IntoResponse {
    match svc.approve(sar_id, body).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/return-for-revision
pub async fn return_for_revision(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<ReturnForRevisionRequest>,
) -> impl IntoResponse {
    match svc.return_for_revision(sar_id, body).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/escalate
pub async fn escalate_sar(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<ReviewActionRequest>,
) -> impl IntoResponse {
    match svc.escalate(sar_id, body).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

// ── Document generation ───────────────────────────────────────────────────────

/// POST /api/admin/compliance/sars/:sar_id/generate
pub async fn generate_document(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.generate_document(sar_id, &caller.user_id).await {
        Ok(doc) => (StatusCode::OK, Json(serde_json::json!({ "document": doc }))).into_response(),
        Err(e) if e.to_string().contains("validation failed") => bad_request(&e.to_string()),
        Err(e) => err(e),
    }
}

/// GET /api/admin/compliance/sars/:sar_id/document
pub async fn get_document(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_document(sar_id, &caller.user_id).await {
        Ok(Some(doc)) => (StatusCode::OK, Json(serde_json::json!({ "document": doc }))).into_response(),
        Ok(None) => not_found(),
        Err(e) => err(e),
    }
}

// ── Filing ────────────────────────────────────────────────────────────────────

/// POST /api/admin/compliance/sars/:sar_id/file
pub async fn file_sar(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<FileRequest>,
) -> impl IntoResponse {
    match svc.file(sar_id, body, &caller.user_id).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) if e.to_string().contains("must be in") || e.to_string().contains("must be generated") => {
            bad_request(&e.to_string())
        }
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/record-acknowledgement
pub async fn record_acknowledgement(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<AcknowledgementRequest>,
) -> impl IntoResponse {
    match svc.record_acknowledgement(sar_id, body).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

/// POST /api/admin/compliance/sars/:sar_id/record-filing-rejection
pub async fn record_filing_rejection(
    State(svc): State<SarState>,
    Path(sar_id): Path<Uuid>,
    Json(body): Json<FilingRejectionRequest>,
) -> impl IntoResponse {
    match svc.record_filing_rejection(sar_id, body).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

// ── Deadline & analytics ──────────────────────────────────────────────────────

/// GET /api/admin/compliance/sars/deadline-status
pub async fn deadline_status(State(svc): State<SarState>) -> impl IntoResponse {
    match svc.get_deadline_status().await {
        Ok(statuses) => (StatusCode::OK, Json(serde_json::json!({ "deadlines": statuses }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/admin/compliance/sars/metrics
pub async fn sar_metrics(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Query(q): Query<MetricsQuery>,
) -> impl IntoResponse {
    match svc.get_metrics(q.from_date, q.to_date, &caller.user_id).await {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/admin/compliance/sars/:sar_id/audit
pub async fn get_audit_log(
    State(svc): State<SarState>,
    Extension(caller): Extension<CallerIdentity>,
    Path(sar_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_audit_log(sar_id, &caller.user_id).await {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!({ "audit": entries }))).into_response(),
        Err(e) => err(e),
    }
}
