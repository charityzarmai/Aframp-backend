//! SAR database repository — all DB access for the SAR module.
//!
//! CONFIDENTIALITY: Every read access is logged to sar_audit_log.
//! No SAR data is written to standard application logs.

use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{
    SarAuditEntry, SarDeadlineStatus, SarMetrics, SarNarrative, SarReport, SarSubject,
    SarTransaction,
};

pub struct SarRepository {
    pool: PgPool,
}

impl SarRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Create ───────────────────────────────────────────────────────────────

    pub async fn create(&self, r: &SarReport) -> Result<SarReport, anyhow::Error> {
        let report = sqlx::query_as!(
            SarReport,
            r#"
            INSERT INTO sar_reports (
                id, sar_type, status, subject_type, detection_method,
                subject_kyc_id, subject_wallet_addresses, suspicious_activity_description,
                activity_start_date, activity_end_date, total_amount_ngn, transaction_count,
                linked_transaction_ids, aml_case_id, aml_risk_score, triggered_rules,
                detecting_officer_id, assigned_investigator_id, investigation_checklist,
                filing_deadline, authority, retention_expires_at, created_at, updated_at
            ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$23
            )
            RETURNING *
            "#,
            r.id,
            r.sar_type,
            r.status,
            r.subject_type,
            r.detection_method,
            r.subject_kyc_id,
            &r.subject_wallet_addresses,
            r.suspicious_activity_description,
            r.activity_start_date,
            r.activity_end_date,
            r.total_amount_ngn,
            r.transaction_count,
            &r.linked_transaction_ids,
            r.aml_case_id,
            r.aml_risk_score,
            r.triggered_rules,
            r.detecting_officer_id,
            r.assigned_investigator_id,
            r.investigation_checklist,
            r.filing_deadline,
            r.authority,
            r.retention_expires_at,
            r.created_at,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(report)
    }

    // ── Read ─────────────────────────────────────────────────────────────────

    pub async fn get(&self, id: Uuid, actor_id: &str) -> Result<Option<SarReport>, anyhow::Error> {
        let report = sqlx::query_as!(SarReport, "SELECT * FROM sar_reports WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await?;
        if report.is_some() {
            self.log_access(id, actor_id, "read", "read").await?;
        }
        Ok(report)
    }

    pub async fn list(
        &self,
        status: Option<&str>,
        subject_type: Option<&str>,
        detection_method: Option<&str>,
        from_date: Option<chrono::DateTime<Utc>>,
        to_date: Option<chrono::DateTime<Utc>>,
        page: i64,
        per_page: i64,
    ) -> Result<Vec<SarReport>, anyhow::Error> {
        let offset = (page - 1) * per_page;
        Ok(sqlx::query_as!(
            SarReport,
            r#"
            SELECT * FROM sar_reports
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::text IS NULL OR subject_type = $2)
              AND ($3::text IS NULL OR detection_method = $3)
              AND ($4::timestamptz IS NULL OR created_at >= $4)
              AND ($5::timestamptz IS NULL OR created_at <= $5)
            ORDER BY created_at DESC
            LIMIT $6 OFFSET $7
            "#,
            status,
            subject_type,
            detection_method,
            from_date,
            to_date,
            per_page,
            offset,
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn find_by_aml_case(&self, aml_case_id: Uuid) -> Result<Option<SarReport>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarReport,
            "SELECT * FROM sar_reports WHERE aml_case_id = $1 LIMIT 1",
            aml_case_id
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    // ── Subjects ─────────────────────────────────────────────────────────────

    pub async fn add_subject(
        &self,
        sar_id: Uuid,
        full_name: &str,
        date_of_birth: Option<NaiveDate>,
        nationality: Option<&str>,
        identification_docs: serde_json::Value,
        address: Option<&str>,
        contact_info: serde_json::Value,
        platform_relationship: &str,
    ) -> Result<SarSubject, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarSubject,
            r#"
            INSERT INTO sar_subjects (id, sar_id, full_name, date_of_birth, nationality,
                identification_docs, address, contact_info, platform_relationship, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW())
            RETURNING *
            "#,
            Uuid::new_v4(),
            sar_id,
            full_name,
            date_of_birth,
            nationality,
            identification_docs,
            address,
            contact_info,
            platform_relationship,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_subjects(&self, sar_id: Uuid) -> Result<Vec<SarSubject>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarSubject,
            "SELECT * FROM sar_subjects WHERE sar_id = $1 ORDER BY created_at ASC",
            sar_id
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ── Transactions ─────────────────────────────────────────────────────────

    pub async fn add_transaction(
        &self,
        sar_id: Uuid,
        transaction_id: Uuid,
        transaction_date: chrono::DateTime<Utc>,
        amount_ngn: rust_decimal::Decimal,
        transaction_type: &str,
        counterparty_details: serde_json::Value,
        suspicious_element: &str,
    ) -> Result<SarTransaction, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarTransaction,
            r#"
            INSERT INTO sar_transactions (id, sar_id, transaction_id, transaction_date,
                amount_ngn, transaction_type, counterparty_details, suspicious_element, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW())
            ON CONFLICT (sar_id, transaction_id) DO UPDATE
                SET suspicious_element = EXCLUDED.suspicious_element
            RETURNING *
            "#,
            Uuid::new_v4(),
            sar_id,
            transaction_id,
            transaction_date,
            amount_ngn,
            transaction_type,
            counterparty_details,
            suspicious_element,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_transactions(&self, sar_id: Uuid) -> Result<Vec<SarTransaction>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarTransaction,
            "SELECT * FROM sar_transactions WHERE sar_id = $1 ORDER BY transaction_date ASC",
            sar_id
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ── Narratives ───────────────────────────────────────────────────────────

    pub async fn add_narrative(
        &self,
        sar_id: Uuid,
        narrative_text: &str,
        author_id: Uuid,
    ) -> Result<SarNarrative, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarNarrative,
            r#"
            INSERT INTO sar_narratives (id, sar_id, version, narrative_text, author_id, created_at)
            VALUES (
                $1, $2,
                COALESCE((SELECT MAX(version) FROM sar_narratives WHERE sar_id = $2), 0) + 1,
                $3, $4, NOW()
            )
            RETURNING *
            "#,
            Uuid::new_v4(),
            sar_id,
            narrative_text,
            author_id,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_narratives(&self, sar_id: Uuid) -> Result<Vec<SarNarrative>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarNarrative,
            "SELECT * FROM sar_narratives WHERE sar_id = $1 ORDER BY version ASC",
            sar_id
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ── State transitions ────────────────────────────────────────────────────

    pub async fn transition(
        &self,
        id: Uuid,
        to_status: &str,
        actor_id: &str,
        action: &str,
        notes: Option<&str>,
        extra_updates: Option<ExtraUpdates>,
    ) -> Result<SarReport, anyhow::Error> {
        let mut tx = self.pool.begin().await?;

        let current: (String,) =
            sqlx::query_as("SELECT status FROM sar_reports WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        let eu = extra_updates.unwrap_or_default();

        sqlx::query!(
            r#"
            UPDATE sar_reports SET
                status = $2,
                reviewing_officer_id = COALESCE($3, reviewing_officer_id),
                approving_officer_id = COALESCE($4, approving_officer_id),
                assigned_investigator_id = COALESCE($5, assigned_investigator_id),
                filing_timestamp = CASE WHEN $2 = 'filed' THEN NOW() ELSE filing_timestamp END,
                filing_method = COALESCE($6, filing_method),
                regulatory_reference_number = COALESCE($7, regulatory_reference_number),
                rejection_reason = COALESCE($8, rejection_reason),
                acknowledged_at = CASE WHEN $2 = 'acknowledged' THEN NOW() ELSE acknowledged_at END,
                acknowledgement_reference = COALESCE($9, acknowledgement_reference),
                generated_document = COALESCE($10, generated_document),
                document_generated_at = CASE WHEN $10 IS NOT NULL THEN NOW() ELSE document_generated_at END,
                investigation_checklist = COALESCE($11::jsonb, investigation_checklist),
                updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            to_status,
            eu.reviewing_officer_id,
            eu.approving_officer_id,
            eu.assigned_investigator_id,
            eu.filing_method,
            eu.regulatory_reference_number,
            eu.rejection_reason,
            eu.acknowledgement_reference,
            eu.generated_document,
            eu.investigation_checklist as Option<serde_json::Value>,
        )
        .execute(&mut *tx)
        .await?;

        let report = sqlx::query_as!(SarReport, "SELECT * FROM sar_reports WHERE id = $1", id)
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query!(
            r#"
            INSERT INTO sar_audit_log (id, sar_id, actor_id, action, from_status, to_status, notes, access_type, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,'write',NOW())
            "#,
            Uuid::new_v4(),
            id,
            actor_id,
            action,
            current.0,
            to_status,
            notes,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    pub async fn update_checklist(
        &self,
        id: Uuid,
        checklist: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            "UPDATE sar_reports SET investigation_checklist = $2, updated_at = NOW() WHERE id = $1",
            id,
            checklist,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Audit log ────────────────────────────────────────────────────────────

    pub async fn get_audit_log(&self, sar_id: Uuid) -> Result<Vec<SarAuditEntry>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarAuditEntry,
            "SELECT * FROM sar_audit_log WHERE sar_id = $1 ORDER BY created_at ASC",
            sar_id
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Log a read access — confidentiality audit trail
    pub async fn log_access(
        &self,
        sar_id: Uuid,
        actor_id: &str,
        action: &str,
        access_type: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
            INSERT INTO sar_audit_log (id, sar_id, actor_id, action, from_status, to_status, access_type, created_at)
            VALUES ($1,$2,$3,$4,'','',$5,NOW())
            "#,
            Uuid::new_v4(),
            sar_id,
            actor_id,
            action,
            access_type,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Deadline management ──────────────────────────────────────────────────

    pub async fn get_deadline_status(&self) -> Result<Vec<SarDeadlineStatus>, anyhow::Error> {
        let today = Utc::now().date_naive();
        let rows = sqlx::query!(
            r#"
            SELECT
                id,
                status,
                filing_deadline,
                EXTRACT(EPOCH FROM (filing_deadline::timestamptz - $1::date::timestamptz))::bigint / 86400 AS days_remaining,
                assigned_investigator_id,
                created_at
            FROM sar_reports
            WHERE status NOT IN ('filed','acknowledged','rejected')
            ORDER BY filing_deadline ASC
            "#,
            today,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| SarDeadlineStatus {
            sar_id: r.id,
            status: r.status,
            filing_deadline: r.filing_deadline,
            days_remaining: r.days_remaining.unwrap_or(0),
            assigned_investigator_id: r.assigned_investigator_id,
            created_at: r.created_at,
        }).collect())
    }

    pub async fn get_overdue_sars(&self) -> Result<Vec<SarReport>, anyhow::Error> {
        let today = Utc::now().date_naive();
        Ok(sqlx::query_as!(
            SarReport,
            r#"
            SELECT * FROM sar_reports
            WHERE filing_deadline < $1
              AND status NOT IN ('filed','acknowledged','rejected')
            ORDER BY filing_deadline ASC
            "#,
            today,
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_approaching_deadline(
        &self,
        days_ahead: i64,
    ) -> Result<Vec<SarReport>, anyhow::Error> {
        let today = Utc::now().date_naive();
        let cutoff = today + chrono::Duration::days(days_ahead);
        Ok(sqlx::query_as!(
            SarReport,
            r#"
            SELECT * FROM sar_reports
            WHERE filing_deadline <= $1
              AND filing_deadline >= $2
              AND status NOT IN ('filed','acknowledged','rejected')
            ORDER BY filing_deadline ASC
            "#,
            cutoff,
            today,
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ── Metrics ──────────────────────────────────────────────────────────────

    pub async fn get_metrics(
        &self,
        from: chrono::DateTime<Utc>,
        to: chrono::DateTime<Utc>,
    ) -> Result<SarMetrics, anyhow::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE TRUE) AS "total_initiated!: i64",
                COUNT(*) FILTER (WHERE status IN ('filed','acknowledged')) AS "total_filed!: i64",
                COUNT(*) FILTER (WHERE rejection_reason IS NOT NULL AND status = 'rejected') AS "total_rejected!: i64",
                COUNT(*) FILTER (WHERE filing_deadline < CURRENT_DATE AND status NOT IN ('filed','acknowledged','rejected')) AS "total_overdue!: i64",
                COALESCE(AVG(EXTRACT(EPOCH FROM (filing_timestamp - created_at))/86400.0) FILTER (WHERE filing_timestamp IS NOT NULL), 0) AS "avg_days!: f64"
            FROM sar_reports
            WHERE created_at BETWEEN $1 AND $2
            "#,
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await?;

        let timeliness = if row.total_filed > 0 {
            let on_time: i64 = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) AS "count!: i64" FROM sar_reports
                WHERE created_at BETWEEN $1 AND $2
                  AND status IN ('filed','acknowledged')
                  AND filing_timestamp::date <= filing_deadline
                "#,
                from,
                to,
            )
            .fetch_one(&self.pool)
            .await?;
            on_time as f64 / row.total_filed as f64
        } else {
            1.0
        };

        let by_method = sqlx::query!(
            r#"
            SELECT detection_method, COUNT(*) AS "count!: i64"
            FROM sar_reports WHERE created_at BETWEEN $1 AND $2
            GROUP BY detection_method
            "#,
            from,
            to,
        )
        .fetch_all(&self.pool)
        .await?;

        let by_subject = sqlx::query!(
            r#"
            SELECT subject_type, COUNT(*) AS "count!: i64"
            FROM sar_reports WHERE created_at BETWEEN $1 AND $2
            GROUP BY subject_type
            "#,
            from,
            to,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(SarMetrics {
            period_from: from,
            period_to: to,
            total_initiated: row.total_initiated,
            total_filed: row.total_filed,
            total_rejected_by_regulator: row.total_rejected,
            total_overdue: row.total_overdue,
            avg_days_detection_to_filing: row.avg_days,
            filing_timeliness_rate: timeliness,
            by_detection_method: serde_json::json!(
                by_method.into_iter().map(|r| (r.detection_method, r.count)).collect::<std::collections::HashMap<_,_>>()
            ),
            by_subject_type: serde_json::json!(
                by_subject.into_iter().map(|r| (r.subject_type, r.count)).collect::<std::collections::HashMap<_,_>>()
            ),
        })
    }
}

/// Optional extra fields to update during a state transition
#[derive(Debug, Default)]
pub struct ExtraUpdates {
    pub reviewing_officer_id: Option<Uuid>,
    pub approving_officer_id: Option<Uuid>,
    pub assigned_investigator_id: Option<Uuid>,
    pub filing_method: Option<String>,
    pub regulatory_reference_number: Option<String>,
    pub rejection_reason: Option<String>,
    pub acknowledgement_reference: Option<String>,
    pub generated_document: Option<String>,
    pub investigation_checklist: Option<serde_json::Value>,
}
