//! SAR document generation — NFIU JSON format and PDF-ready JSON for record keeping.
//!
//! NFIU (Nigerian Financial Intelligence Unit) accepts JSON-structured SAR reports.
//! We also produce a human-readable JSON for PDF generation / record keeping.

use chrono::Utc;

use super::models::{SarNarrative, SarReport, SarSubject, SarTransaction};

/// Generate the NFIU-format SAR document (JSON string).
/// Validates required fields and returns an error if any are missing.
pub fn generate_nfiu_document(
    report: &SarReport,
    subjects: &[SarSubject],
    transactions: &[SarTransaction],
    narratives: &[SarNarrative],
    filer_institution: &str,
    filer_rc_number: &str,
) -> Result<String, Vec<String>> {
    let mut errors = Vec::new();

    if report.suspicious_activity_description.trim().is_empty() {
        errors.push("suspicious_activity_description is required".into());
    }
    if subjects.is_empty() {
        errors.push("at least one subject is required".into());
    }
    if transactions.is_empty() {
        errors.push("at least one transaction is required".into());
    }
    if narratives.is_empty() {
        errors.push("narrative is required".into());
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let latest_narrative = narratives.last().unwrap();

    let doc = serde_json::json!({
        "report_type": "SAR",
        "schema_version": "NFIU-SAR-v2",
        "report_id": report.id,
        "filing_institution": {
            "name": filer_institution,
            "rc_number": filer_rc_number,
            "filing_date": Utc::now().format("%Y-%m-%d").to_string(),
        },
        "sar_type": report.sar_type,
        "detection_method": report.detection_method,
        "activity_period": {
            "start": report.activity_start_date.to_string(),
            "end": report.activity_end_date.to_string(),
        },
        "total_amount_ngn": report.total_amount_ngn,
        "transaction_count": report.transaction_count,
        "subjects": subjects.iter().map(|s| serde_json::json!({
            "full_name": s.full_name,
            "date_of_birth": s.date_of_birth,
            "nationality": s.nationality,
            "identification_docs": s.identification_docs,
            "address": s.address,
            "contact_info": s.contact_info,
            "platform_relationship": s.platform_relationship,
        })).collect::<Vec<_>>(),
        "transactions": transactions.iter().map(|t| serde_json::json!({
            "transaction_id": t.transaction_id,
            "date": t.transaction_date.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "amount_ngn": t.amount_ngn,
            "type": t.transaction_type,
            "counterparty": t.counterparty_details,
            "suspicious_element": t.suspicious_element,
        })).collect::<Vec<_>>(),
        "narrative": latest_narrative.narrative_text,
        "narrative_version": latest_narrative.version,
        "suspicious_activity_description": report.suspicious_activity_description,
        "aml_case_id": report.aml_case_id,
        "triggered_rules": report.triggered_rules,
        "authority": report.authority,
        "generated_at": Utc::now().to_rfc3339(),
    });

    Ok(serde_json::to_string_pretty(&doc).unwrap_or_default())
}

/// Validate a generated document string against NFIU required fields.
/// Returns list of validation errors (empty = valid).
pub fn validate_document(doc_json: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(doc_json) else {
        return vec!["document is not valid JSON".into()];
    };

    for field in &[
        "report_id",
        "filing_institution",
        "sar_type",
        "subjects",
        "transactions",
        "narrative",
        "suspicious_activity_description",
        "activity_period",
        "total_amount_ngn",
    ] {
        if v.get(field).is_none() {
            errors.push(format!("required field missing: {field}"));
        }
    }

    if let Some(subjects) = v.get("subjects").and_then(|s| s.as_array()) {
        if subjects.is_empty() {
            errors.push("subjects array must not be empty".into());
        }
    }
    if let Some(txns) = v.get("transactions").and_then(|t| t.as_array()) {
        if txns.is_empty() {
            errors.push("transactions array must not be empty".into());
        }
    }

    errors
}
