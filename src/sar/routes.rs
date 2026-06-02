//! SAR route definitions — all under /api/admin/compliance/sars

use axum::{
    routing::{get, patch, post},
    Router,
};

use super::handlers::*;

pub fn sar_routes(state: SarState) -> Router {
    Router::new()
        // Initiation
        .route("/", post(create_sar))
        // Investigation workflow
        .route("/", get(list_sars))
        .route("/deadline-status", get(deadline_status))
        .route("/metrics", get(sar_metrics))
        .route("/:sar_id", get(get_sar))
        .route("/:sar_id/transactions", post(add_transaction))
        .route("/:sar_id/subjects", post(add_subject))
        .route("/:sar_id/narrative", patch(update_narrative))
        .route("/:sar_id/checklist", patch(update_checklist))
        .route("/:sar_id/submit-for-review", post(submit_for_review))
        // Review / approval
        .route("/:sar_id/approve", post(approve_sar))
        .route("/:sar_id/return-for-revision", post(return_for_revision))
        .route("/:sar_id/escalate", post(escalate_sar))
        // Document generation
        .route("/:sar_id/generate", post(generate_document))
        .route("/:sar_id/document", get(get_document))
        // Filing
        .route("/:sar_id/file", post(file_sar))
        .route("/:sar_id/record-acknowledgement", post(record_acknowledgement))
        .route("/:sar_id/record-filing-rejection", post(record_filing_rejection))
        // Audit
        .route("/:sar_id/audit", get(get_audit_log))
        .with_state(state)
}
