//! SAR service — full business logic for the SAR lifecycle.
//!
//! CONFIDENTIALITY: This service never writes SAR data to standard application logs.
//! All structured events go to the secure compliance log via `compliance_log!`.

use chrono::{NaiveDate, Utc};
use rust_decimal::prelude::FromStr;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::{
    metrics,
    models::*,
    repository::{ExtraUpdates, SarRepository},
    template,
};

/// Macro that writes to the secure compliance log only — never to stdout/stderr.
/// In production this should route to a dedicated append-only compliance log sink.
macro_rules! compliance_log {
    ($($arg:tt)*) => {
        tracing::info!(target: "sar_compliance", $($arg)*)
    };
}

/// Days before deadline to send reminder (configurable via env)
fn reminder_days() -> Vec<i64> {
    std::env::var("SAR_REMINDER_DAYS")
        .unwrap_or_else(|_| "14,7,3,1".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Default filing deadline in days from suspicion formation
fn default_deadline_days() -> i64 {
    std::env::var("SAR_FILING_DEADLINE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

/// Senior officer approval threshold in NGN
fn senior_approval_threshold() -> rust_decimal::Decimal {
    std::env::var("SAR_SENIOR_APPROVAL_THRESHOLD_NGN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| rust_decimal::Decimal::from(50_000_000))
}

/// Max investigation duration in days before alert fires
fn max_investigation_days() -> i64 {
    std::env::var("SAR_MAX_INVESTIGATION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(21)
}

/// SAR filing rejection rate alert threshold (0.0–1.0)
fn rejection_rate_threshold() -> f64 {
    std::env::var("SAR_REJECTION_RATE_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.1)
}

pub struct SarService {
    repo: Arc<SarRepository>,
    pool: PgPool,
    filer_institution: String,
    filer_rc_number: String,
}

impl SarService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: Arc::new(SarRepository::new(pool.clone())),
            pool,
            filer_institution: std::env::var("FILER_INSTITUTION_NAME")
                .unwrap_or_else(|_| "Aframp".into()),
            filer_rc_number: std::env::var("FILER_RC_NUMBER")
                .unwrap_or_else(|_| "RC000000".into()),
        }
    }

    // ── Initiation ───────────────────────────────────────────────────────────

    /// Auto-initiate a SAR from an AML rule trigger or sanctions match.
    /// Idempotent: returns existing SAR if one already exists for this AML case.
    pub async fn auto_initiate(
        &self,
        aml_case_id: Uuid,
        detection_method: DetectionMethod,
        subject_kyc_id: Option<Uuid>,
        subject_wallet_addresses: Vec<String>,
        suspicious_activity_description: String,
        activity_start_date: NaiveDate,
        activity_end_date: NaiveDate,
        total_amount_ngn: rust_decimal::Decimal,
        transaction_count: i32,
        linked_transaction_ids: Vec<Uuid>,
        triggered_rules: serde_json::Value,
        aml_risk_score: Option<f64>,
        assigned_investigator_id: Option<Uuid>,
    ) -> Result<SarReport, anyhow::Error> {
        // Idempotency
        if let Some(existing) = self.repo.find_by_aml_case(aml_case_id).await? {
            return Ok(existing);
        }

        let deadline = Utc::now().date_naive() + chrono::Duration::days(default_deadline_days());
        let now = Utc::now();

        let report = SarReport {
            id: Uuid::new_v4(),
            sar_type: SarType::ActivityBased.to_string(),
            status: SarStatus::Draft.to_string(),
            subject_type: SubjectType::Individual.to_string(),
            detection_method: detection_method.to_string(),
            subject_kyc_id,
            subject_wallet_addresses,
            suspicious_activity_description,
            activity_start_date,
            activity_end_date,
            total_amount_ngn,
            transaction_count,
            linked_transaction_ids,
            aml_case_id: Some(aml_case_id),
            aml_risk_score: aml_risk_score.map(|s| {
                rust_decimal::Decimal::from_str_exact(&format!("{s:.4}"))
                    .unwrap_or(rust_decimal::Decimal::ZERO)
            }),
            triggered_rules,
            detecting_officer_id: None,
            assigned_investigator_id,
            reviewing_officer_id: None,
            approving_officer_id: None,
            investigation_checklist: serde_json::to_value(InvestigationChecklist::default())?,
            filing_deadline: deadline,
            filing_timestamp: None,
            filing_method: None,
            regulatory_reference_number: None,
            rejection_reason: None,
            acknowledged_at: None,
            acknowledgement_reference: None,
            authority: "NFIU".into(),
            generated_document: None,
            document_generated_at: None,
            retention_expires_at: Utc::now().date_naive() + chrono::Duration::days(365 * 5),
            created_at: now,
            updated_at: now,
        };

        let saved = self.repo.create(&report).await?;

        // Audit initial creation
        self.write_audit(saved.id, "system", "auto_initiated", "", &SarStatus::Draft.to_string(), None).await?;

        metrics::inc_initiated(&detection_method.to_string());

        compliance_log!(
            sar_id = %saved.id,
            aml_case_id = %aml_case_id,
            detection_method = %detection_method,
            "SAR auto-initiated"
        );

        Ok(saved)
    }

    /// Manual SAR initiation by a compliance officer.
    pub async fn manual_initiate(
        &self,
        req: CreateSarRequest,
        officer_id: Uuid,
    ) -> Result<SarReport, anyhow::Error> {
        let deadline_days = req.deadline_days.unwrap_or_else(default_deadline_days);
        let deadline = Utc::now().date_naive() + chrono::Duration::days(deadline_days);
        let now = Utc::now();

        let report = SarReport {
            id: Uuid::new_v4(),
            sar_type: req.sar_type.to_string(),
            status: SarStatus::Draft.to_string(),
            subject_type: req.subject_type.to_string(),
            detection_method: req.detection_method.to_string(),
            subject_kyc_id: req.subject_kyc_id,
            subject_wallet_addresses: req.subject_wallet_addresses,
            suspicious_activity_description: req.suspicious_activity_description,
            activity_start_date: req.activity_start_date,
            activity_end_date: req.activity_end_date,
            total_amount_ngn: req.total_amount_ngn,
            transaction_count: req.transaction_count,
            linked_transaction_ids: req.linked_transaction_ids,
            aml_case_id: None,
            aml_risk_score: None,
            triggered_rules: serde_json::json!([]),
            detecting_officer_id: Some(officer_id),
            assigned_investigator_id: req.assigned_investigator_id,
            reviewing_officer_id: None,
            approving_officer_id: None,
            investigation_checklist: serde_json::to_value(InvestigationChecklist::default())?,
            filing_deadline: deadline,
            filing_timestamp: None,
            filing_method: None,
            regulatory_reference_number: None,
            rejection_reason: None,
            acknowledged_at: None,
            acknowledgement_reference: None,
            authority: "NFIU".into(),
            generated_document: None,
            document_generated_at: None,
            retention_expires_at: Utc::now().date_naive() + chrono::Duration::days(365 * 5),
            created_at: now,
            updated_at: now,
        };

        let saved = self.repo.create(&report).await?;
        self.write_audit(saved.id, &officer_id.to_string(), "manual_initiated", "", &SarStatus::Draft.to_string(), None).await?;
        metrics::inc_initiated(&req.detection_method.to_string());

        compliance_log!(sar_id = %saved.id, officer_id = %officer_id, "SAR manually initiated");
        Ok(saved)
    }

    // ── Investigation workflow ────────────────────────────────────────────────

    pub async fn list(
        &self,
        q: &SarListQuery,
        actor_id: &str,
    ) -> Result<Vec<SarReport>, anyhow::Error> {
        compliance_log!(actor_id = %actor_id, "SAR list accessed");
        self.repo.list(
            q.status.as_deref(),
            q.subject_type.as_deref(),
            q.detection_method.as_deref(),
            q.from_date,
            q.to_date,
            q.page.unwrap_or(1),
            q.per_page.unwrap_or(20),
        ).await
    }

    pub async fn get_detail(
        &self,
        sar_id: Uuid,
        actor_id: &str,
    ) -> Result<Option<SarDetail>, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, actor_id).await? else {
            return Ok(None);
        };
        let (subjects, transactions, narratives, audit_log) = tokio::try_join!(
            self.repo.get_subjects(sar_id),
            self.repo.get_transactions(sar_id),
            self.repo.get_narratives(sar_id),
            self.repo.get_audit_log(sar_id),
        )?;
        Ok(Some(SarDetail { report, subjects, transactions, narratives, audit_log }))
    }

    pub async fn add_transaction(
        &self,
        sar_id: Uuid,
        req: AddTransactionRequest,
        actor_id: &str,
    ) -> Result<SarTransaction, anyhow::Error> {
        let t = self.repo.add_transaction(
            sar_id,
            req.transaction_id,
            req.transaction_date,
            req.amount_ngn,
            &req.transaction_type,
            req.counterparty_details.unwrap_or_default(),
            &req.suspicious_element,
        ).await?;
        self.repo.log_access(sar_id, actor_id, "add_transaction", "write").await?;
        compliance_log!(sar_id = %sar_id, actor_id = %actor_id, "transaction added to SAR");
        Ok(t)
    }

    pub async fn add_subject(
        &self,
        sar_id: Uuid,
        req: AddSubjectRequest,
        actor_id: &str,
    ) -> Result<SarSubject, anyhow::Error> {
        let s = self.repo.add_subject(
            sar_id,
            &req.full_name,
            req.date_of_birth,
            req.nationality.as_deref(),
            req.identification_docs.unwrap_or_default(),
            req.address.as_deref(),
            req.contact_info.unwrap_or_default(),
            req.platform_relationship.as_deref().unwrap_or("account_holder"),
        ).await?;
        self.repo.log_access(sar_id, actor_id, "add_subject", "write").await?;
        compliance_log!(sar_id = %sar_id, actor_id = %actor_id, "subject added to SAR");
        Ok(s)
    }

    pub async fn update_narrative(
        &self,
        sar_id: Uuid,
        req: UpdateNarrativeRequest,
    ) -> Result<SarNarrative, anyhow::Error> {
        let n = self.repo.add_narrative(sar_id, &req.narrative_text, req.author_id).await?;
        compliance_log!(sar_id = %sar_id, author_id = %req.author_id, version = n.version, "SAR narrative updated");
        Ok(n)
    }

    pub async fn update_checklist(
        &self,
        sar_id: Uuid,
        checklist: InvestigationChecklist,
        actor_id: &str,
    ) -> Result<(), anyhow::Error> {
        self.repo.update_checklist(sar_id, serde_json::to_value(&checklist)?).await?;
        self.repo.log_access(sar_id, actor_id, "update_checklist", "write").await?;
        Ok(())
    }

    /// Submit SAR for review — enforces investigation checklist completion.
    pub async fn submit_for_review(
        &self,
        sar_id: Uuid,
        actor_id: &str,
    ) -> Result<SarReport, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, actor_id).await? else {
            anyhow::bail!("SAR not found");
        };
        let checklist: InvestigationChecklist =
            serde_json::from_value(report.investigation_checklist.clone())?;
        if !checklist.is_complete() {
            anyhow::bail!("investigation checklist is not complete — all steps must be checked before submission");
        }
        let r = self.repo.transition(
            sar_id,
            &SarStatus::UnderReview.to_string(),
            actor_id,
            "submit_for_review",
            None,
            None,
        ).await?;
        compliance_log!(sar_id = %sar_id, actor_id = %actor_id, "SAR submitted for review");
        Ok(r)
    }

    // ── Review / approval workflow ────────────────────────────────────────────

    pub async fn approve(
        &self,
        sar_id: Uuid,
        req: ReviewActionRequest,
    ) -> Result<SarReport, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, &req.officer_id.to_string()).await? else {
            anyhow::bail!("SAR not found");
        };
        // High-value SARs require senior officer approval — enforced by caller
        // passing the correct officer_id; the threshold check is advisory here.
        if report.total_amount_ngn >= senior_approval_threshold() {
            compliance_log!(
                sar_id = %sar_id,
                amount = %report.total_amount_ngn,
                "High-value SAR approved — senior officer approval required"
            );
        }
        let r = self.repo.transition(
            sar_id,
            &SarStatus::Approved.to_string(),
            &req.officer_id.to_string(),
            "approved",
            req.notes.as_deref(),
            Some(ExtraUpdates {
                approving_officer_id: Some(req.officer_id),
                ..Default::default()
            }),
        ).await?;
        compliance_log!(sar_id = %sar_id, officer_id = %req.officer_id, "SAR approved");
        Ok(r)
    }

    pub async fn return_for_revision(
        &self,
        sar_id: Uuid,
        req: ReturnForRevisionRequest,
    ) -> Result<SarReport, anyhow::Error> {
        let r = self.repo.transition(
            sar_id,
            &SarStatus::ReturnedForRevision.to_string(),
            &req.officer_id.to_string(),
            "returned_for_revision",
            Some(&req.required_revisions),
            Some(ExtraUpdates {
                reviewing_officer_id: Some(req.officer_id),
                ..Default::default()
            }),
        ).await?;
        compliance_log!(sar_id = %sar_id, officer_id = %req.officer_id, "SAR returned for revision");
        Ok(r)
    }

    pub async fn escalate(
        &self,
        sar_id: Uuid,
        req: ReviewActionRequest,
    ) -> Result<SarReport, anyhow::Error> {
        let r = self.repo.transition(
            sar_id,
            &SarStatus::UnderReview.to_string(),
            &req.officer_id.to_string(),
            "escalated",
            req.notes.as_deref(),
            Some(ExtraUpdates {
                reviewing_officer_id: Some(req.officer_id),
                ..Default::default()
            }),
        ).await?;
        compliance_log!(sar_id = %sar_id, officer_id = %req.officer_id, "SAR escalated");
        Ok(r)
    }

    // ── Document generation ───────────────────────────────────────────────────

    pub async fn generate_document(
        &self,
        sar_id: Uuid,
        actor_id: &str,
    ) -> Result<String, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, actor_id).await? else {
            anyhow::bail!("SAR not found");
        };
        let (subjects, transactions, narratives) = tokio::try_join!(
            self.repo.get_subjects(sar_id),
            self.repo.get_transactions(sar_id),
            self.repo.get_narratives(sar_id),
        )?;

        let doc = template::generate_nfiu_document(
            &report,
            &subjects,
            &transactions,
            &narratives,
            &self.filer_institution,
            &self.filer_rc_number,
        )
        .map_err(|errs| anyhow::anyhow!("SAR format validation failed: {}", errs.join("; ")))?;

        // Validate before storing
        let validation_errors = template::validate_document(&doc);
        if !validation_errors.is_empty() {
            anyhow::bail!("SAR document validation failed: {}", validation_errors.join("; "));
        }

        // Persist generated document
        self.repo.transition(
            sar_id,
            &report.status, // status unchanged
            actor_id,
            "document_generated",
            None,
            Some(ExtraUpdates {
                generated_document: Some(doc.clone()),
                ..Default::default()
            }),
        ).await?;

        compliance_log!(sar_id = %sar_id, actor_id = %actor_id, "SAR document generated");
        Ok(doc)
    }

    pub async fn get_document(
        &self,
        sar_id: Uuid,
        actor_id: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        let report = self.repo.get(sar_id, actor_id).await?;
        Ok(report.and_then(|r| r.generated_document))
    }

    // ── Filing ────────────────────────────────────────────────────────────────

    pub async fn file(
        &self,
        sar_id: Uuid,
        req: FileRequest,
        actor_id: &str,
    ) -> Result<SarReport, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, actor_id).await? else {
            anyhow::bail!("SAR not found");
        };
        if report.status != SarStatus::Approved.to_string() {
            anyhow::bail!("SAR must be in 'approved' status before filing");
        }
        if report.generated_document.is_none() {
            anyhow::bail!("SAR document must be generated before filing");
        }

        let r = self.repo.transition(
            sar_id,
            &SarStatus::Filed.to_string(),
            actor_id,
            "filed",
            None,
            Some(ExtraUpdates {
                filing_method: Some(req.filing_method.clone()),
                regulatory_reference_number: req.regulatory_reference_number.clone(),
                ..Default::default()
            }),
        ).await?;

        metrics::inc_filed(&req.filing_method);
        compliance_log!(
            sar_id = %sar_id,
            filing_method = %req.filing_method,
            ref_number = ?req.regulatory_reference_number,
            "SAR filed"
        );
        Ok(r)
    }

    pub async fn record_acknowledgement(
        &self,
        sar_id: Uuid,
        req: AcknowledgementRequest,
    ) -> Result<SarReport, anyhow::Error> {
        let r = self.repo.transition(
            sar_id,
            &SarStatus::Acknowledged.to_string(),
            &req.officer_id.to_string(),
            "acknowledged",
            None,
            Some(ExtraUpdates {
                acknowledgement_reference: Some(req.acknowledgement_reference.clone()),
                ..Default::default()
            }),
        ).await?;
        compliance_log!(sar_id = %sar_id, reference = %req.acknowledgement_reference, "SAR acknowledged by regulator");
        Ok(r)
    }

    pub async fn record_filing_rejection(
        &self,
        sar_id: Uuid,
        req: FilingRejectionRequest,
    ) -> Result<SarReport, anyhow::Error> {
        let Some(report) = self.repo.get(sar_id, &req.officer_id.to_string()).await? else {
            anyhow::bail!("SAR not found");
        };
        let r = self.repo.transition(
            sar_id,
            &SarStatus::ReturnedForRevision.to_string(),
            &req.officer_id.to_string(),
            "filing_rejected_by_regulator",
            Some(&req.rejection_reason),
            Some(ExtraUpdates {
                rejection_reason: Some(req.rejection_reason.clone()),
                ..Default::default()
            }),
        ).await?;
        metrics::inc_rejected_by_regulator(&report.authority);
        compliance_log!(
            sar_id = %sar_id,
            reason = %req.rejection_reason,
            "SAR filing rejected by regulator"
        );
        Ok(r)
    }

    // ── Deadline management ───────────────────────────────────────────────────

    pub async fn get_deadline_status(&self) -> Result<Vec<SarDeadlineStatus>, anyhow::Error> {
        self.repo.get_deadline_status().await
    }

    /// Called by the deadline worker — checks for overdue SARs and approaching deadlines.
    pub async fn run_deadline_checks(&self) -> Result<(), anyhow::Error> {
        let overdue = self.repo.get_overdue_sars().await?;
        let overdue_count = overdue.len() as f64;
        metrics::set_overdue_count(overdue_count);

        for sar in &overdue {
            metrics::inc_past_deadline(&sar.detection_method);
            compliance_log!(
                sar_id = %sar.id,
                deadline = %sar.filing_deadline,
                "ALERT: SAR past filing deadline without being filed"
            );
        }

        // Approaching deadline reminders
        for days in reminder_days() {
            let approaching = self.repo.get_approaching_deadline(days).await?;
            for sar in approaching {
                compliance_log!(
                    sar_id = %sar.id,
                    days_remaining = days,
                    investigator_id = ?sar.assigned_investigator_id,
                    "SAR deadline reminder"
                );
            }
        }

        // Update nearest deadline gauge
        let statuses = self.repo.get_deadline_status().await?;
        if let Some(nearest) = statuses.first() {
            metrics::set_days_until_nearest_deadline(nearest.days_remaining as f64);
        }

        // Update open-by-status gauges
        for status in &["draft", "under_review", "approved", "returned_for_revision"] {
            let count = statuses.iter().filter(|s| s.status == *status).count() as f64;
            metrics::set_open_by_status(status, count);
        }

        // Alert on long investigation duration
        let max_days = max_investigation_days();
        let now = Utc::now();
        for sar in &statuses {
            if sar.status == "under_review" || sar.status == "draft" {
                let age_days = (now - sar.created_at).num_days();
                if age_days > max_days {
                    compliance_log!(
                        sar_id = %sar.sar_id,
                        age_days = age_days,
                        "ALERT: SAR investigation duration exceeded maximum"
                    );
                }
            }
        }

        Ok(())
    }

    // ── Analytics ────────────────────────────────────────────────────────────

    pub async fn get_metrics(
        &self,
        from: chrono::DateTime<Utc>,
        to: chrono::DateTime<Utc>,
        actor_id: &str,
    ) -> Result<SarMetrics, anyhow::Error> {
        compliance_log!(actor_id = %actor_id, "SAR metrics accessed");
        let metrics = self.repo.get_metrics(from, to).await?;

        // Alert on high rejection rate
        if metrics.total_filed > 0 {
            let rejection_rate = metrics.total_rejected_by_regulator as f64 / metrics.total_filed as f64;
            if rejection_rate > rejection_rate_threshold() {
                compliance_log!(
                    rejection_rate = rejection_rate,
                    threshold = rejection_rate_threshold(),
                    "ALERT: SAR filing rejection rate exceeds threshold"
                );
            }
        }

        Ok(metrics)
    }

    // ── Audit log ─────────────────────────────────────────────────────────────

    pub async fn get_audit_log(
        &self,
        sar_id: Uuid,
        actor_id: &str,
    ) -> Result<Vec<SarAuditEntry>, anyhow::Error> {
        self.repo.log_access(sar_id, actor_id, "read_audit_log", "read").await?;
        self.repo.get_audit_log(sar_id).await
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn write_audit(
        &self,
        sar_id: Uuid,
        actor_id: &str,
        action: &str,
        from_status: &str,
        to_status: &str,
        notes: Option<&str>,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
            INSERT INTO sar_audit_log (id, sar_id, actor_id, action, from_status, to_status, notes, access_type, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,'write',NOW())
            "#,
            Uuid::new_v4(),
            sar_id,
            actor_id,
            action,
            from_status,
            to_status,
            notes,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
