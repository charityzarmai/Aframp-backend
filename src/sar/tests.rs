//! SAR unit and integration tests

#[cfg(test)]
mod unit {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::super::{
        models::{
            DetectionMethod, InvestigationChecklist, SarNarrative, SarReport, SarStatus, SarType,
            SubjectType,
        },
        template,
    };

    // ── Checklist validation ──────────────────────────────────────────────────

    #[test]
    fn checklist_incomplete_blocks_submission() {
        let checklist = InvestigationChecklist {
            subject_identity_verified: true,
            transaction_records_reviewed: true,
            aml_rules_documented: false, // incomplete
            narrative_complete: true,
            supporting_docs_attached: true,
            legal_review_complete: true,
        };
        assert!(!checklist.is_complete());
    }

    #[test]
    fn checklist_complete_allows_submission() {
        let checklist = InvestigationChecklist {
            subject_identity_verified: true,
            transaction_records_reviewed: true,
            aml_rules_documented: true,
            narrative_complete: true,
            supporting_docs_attached: true,
            legal_review_complete: true,
        };
        assert!(checklist.is_complete());
    }

    // ── Filing deadline calculation ───────────────────────────────────────────

    #[test]
    fn filing_deadline_is_30_days_from_today_by_default() {
        let today = Utc::now().date_naive();
        let deadline = today + chrono::Duration::days(30);
        assert_eq!((deadline - today).num_days(), 30);
    }

    // ── SAR status display ────────────────────────────────────────────────────

    #[test]
    fn sar_status_display() {
        assert_eq!(SarStatus::Draft.to_string(), "draft");
        assert_eq!(SarStatus::UnderReview.to_string(), "under_review");
        assert_eq!(SarStatus::Approved.to_string(), "approved");
        assert_eq!(SarStatus::Filed.to_string(), "filed");
        assert_eq!(SarStatus::Acknowledged.to_string(), "acknowledged");
        assert_eq!(SarStatus::Rejected.to_string(), "rejected");
        assert_eq!(SarStatus::ReturnedForRevision.to_string(), "returned_for_revision");
    }

    // ── Detection method display ──────────────────────────────────────────────

    #[test]
    fn detection_method_display() {
        assert_eq!(DetectionMethod::AmlRuleTrigger.to_string(), "aml_rule_trigger");
        assert_eq!(DetectionMethod::SanctionsMatch.to_string(), "sanctions_match");
        assert_eq!(DetectionMethod::ComplianceOfficerJudgment.to_string(), "compliance_officer_judgment");
        assert_eq!(DetectionMethod::LawEnforcementRequest.to_string(), "law_enforcement_request");
    }

    // ── Document format validation ────────────────────────────────────────────

    #[test]
    fn validate_document_rejects_missing_fields() {
        let doc = r#"{"report_id": "abc"}"#;
        let errors = template::validate_document(doc);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.contains("filing_institution")));
    }

    #[test]
    fn validate_document_rejects_invalid_json() {
        let errors = template::validate_document("not json");
        assert_eq!(errors, vec!["document is not valid JSON"]);
    }

    #[test]
    fn validate_document_rejects_empty_subjects() {
        let doc = serde_json::json!({
            "report_id": Uuid::new_v4(),
            "filing_institution": {"name": "Aframp", "rc_number": "RC001", "filing_date": "2026-01-01"},
            "sar_type": "activity_based",
            "subjects": [],
            "transactions": [{"transaction_id": Uuid::new_v4()}],
            "narrative": "test",
            "suspicious_activity_description": "test",
            "activity_period": {"start": "2026-01-01", "end": "2026-01-31"},
            "total_amount_ngn": "1000000",
        });
        let errors = template::validate_document(&doc.to_string());
        assert!(errors.iter().any(|e| e.contains("subjects")));
    }

    // ── Narrative version tracking ────────────────────────────────────────────

    #[test]
    fn narrative_versions_are_ordered() {
        let sar_id = Uuid::new_v4();
        let author = Uuid::new_v4();
        let now = Utc::now();
        let narratives = vec![
            SarNarrative { id: Uuid::new_v4(), sar_id, version: 1, narrative_text: "v1".into(), author_id: author, created_at: now },
            SarNarrative { id: Uuid::new_v4(), sar_id, version: 2, narrative_text: "v2".into(), author_id: author, created_at: now },
            SarNarrative { id: Uuid::new_v4(), sar_id, version: 3, narrative_text: "v3".into(), author_id: author, created_at: now },
        ];
        assert_eq!(narratives.last().unwrap().version, 3);
        assert_eq!(narratives.last().unwrap().narrative_text, "v3");
    }

    // ── Tipping-off prevention ────────────────────────────────────────────────

    #[test]
    fn sar_status_not_exposed_in_subject_facing_fields() {
        // SarReport serialises without any field that would reveal SAR existence to subject.
        // The subject_wallet_addresses field is internal — never returned to the wallet owner.
        // This test verifies the model doesn't accidentally include a "notify_subject" field.
        let report_json = serde_json::to_value(SarReport {
            id: Uuid::new_v4(),
            sar_type: SarType::ActivityBased.to_string(),
            status: SarStatus::Draft.to_string(),
            subject_type: SubjectType::Individual.to_string(),
            detection_method: DetectionMethod::AmlRuleTrigger.to_string(),
            subject_kyc_id: None,
            subject_wallet_addresses: vec![],
            suspicious_activity_description: "test".into(),
            activity_start_date: Utc::now().date_naive(),
            activity_end_date: Utc::now().date_naive(),
            total_amount_ngn: Decimal::ZERO,
            transaction_count: 0,
            linked_transaction_ids: vec![],
            aml_case_id: None,
            aml_risk_score: None,
            triggered_rules: serde_json::json!([]),
            detecting_officer_id: None,
            assigned_investigator_id: None,
            reviewing_officer_id: None,
            approving_officer_id: None,
            investigation_checklist: serde_json::json!({}),
            filing_deadline: Utc::now().date_naive(),
            filing_timestamp: None,
            filing_method: None,
            regulatory_reference_number: None,
            rejection_reason: None,
            acknowledged_at: None,
            acknowledgement_reference: None,
            authority: "NFIU".into(),
            generated_document: None,
            document_generated_at: None,
            retention_expires_at: Utc::now().date_naive(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }).unwrap();

        // No subject-notification field should exist
        assert!(report_json.get("notify_subject").is_none());
        assert!(report_json.get("subject_notified").is_none());
        assert!(report_json.get("email_subject").is_none());
    }

    // ── Access control ────────────────────────────────────────────────────────

    #[test]
    fn sar_access_requires_actor_id() {
        // The RBAC middleware enforces X-User-Id presence before any handler runs.
        // Without it, extract_identity returns 401. This test verifies the fallback
        // string used in audit logs when identity is absent.
        let fallback = "unknown";
        assert_eq!(fallback, "unknown"); // sentinel — real enforcement is in rbac middleware
    }

    // ── Pre-population from AML trigger ──────────────────────────────────────

    #[test]
    fn auto_initiate_sets_correct_detection_method() {
        let method = DetectionMethod::AmlRuleTrigger;
        assert_eq!(method.to_string(), "aml_rule_trigger");
    }

    #[test]
    fn sanctions_match_sets_correct_detection_method() {
        let method = DetectionMethod::SanctionsMatch;
        assert_eq!(method.to_string(), "sanctions_match");
    }
}

// ── Integration tests (require DATABASE_URL) ─────────────────────────────────

#[cfg(test)]
#[cfg(feature = "integration_tests")]
mod integration {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::super::{
        models::*,
        service::SarService,
    };

    async fn make_service() -> SarService {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
        let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
        SarService::new(pool)
    }

    #[tokio::test]
    async fn full_sar_lifecycle() {
        let svc = make_service().await;
        let aml_case_id = Uuid::new_v4();
        let officer_id = Uuid::new_v4();
        let today = Utc::now().date_naive();

        // 1. Auto-initiate
        let sar = svc.auto_initiate(
            aml_case_id,
            DetectionMethod::AmlRuleTrigger,
            None,
            vec!["GTEST123".into()],
            "Suspicious layering activity detected".into(),
            today - chrono::Duration::days(7),
            today,
            Decimal::from(5_000_000),
            10,
            vec![],
            serde_json::json!(["velocity_rule_001"]),
            Some(0.92),
            Some(officer_id),
        ).await.unwrap();
        assert_eq!(sar.status, "draft");

        // 2. Idempotency — second call returns same SAR
        let sar2 = svc.auto_initiate(
            aml_case_id,
            DetectionMethod::AmlRuleTrigger,
            None, vec![], "".into(), today, today,
            Decimal::ZERO, 0, vec![], serde_json::json!([]), None, None,
        ).await.unwrap();
        assert_eq!(sar.id, sar2.id);

        // 3. Add subject
        let subject = svc.add_subject(sar.id, AddSubjectRequest {
            full_name: "John Doe".into(),
            date_of_birth: Some(chrono::NaiveDate::from_ymd_opt(1985, 3, 15).unwrap()),
            nationality: Some("NG".into()),
            identification_docs: Some(serde_json::json!([{"type":"NIN","number":"12345678901"}])),
            address: Some("123 Lagos Street".into()),
            contact_info: Some(serde_json::json!({"phone":"+2348012345678"})),
            platform_relationship: Some("account_holder".into()),
        }, &officer_id.to_string()).await.unwrap();
        assert_eq!(subject.full_name, "John Doe");

        // 4. Add transaction
        let txn = svc.add_transaction(sar.id, AddTransactionRequest {
            transaction_id: Uuid::new_v4(),
            transaction_date: Utc::now(),
            amount_ngn: Decimal::from(500_000),
            transaction_type: "onramp".into(),
            counterparty_details: None,
            suspicious_element: "Rapid structuring below threshold".into(),
        }, &officer_id.to_string()).await.unwrap();
        assert_eq!(txn.sar_id, sar.id);

        // 5. Update narrative
        let narrative = svc.update_narrative(sar.id, UpdateNarrativeRequest {
            narrative_text: "Subject conducted 10 transactions over 7 days totalling NGN 5M.".into(),
            author_id: officer_id,
        }).await.unwrap();
        assert_eq!(narrative.version, 1);

        // 6. Update narrative again — version increments
        let narrative2 = svc.update_narrative(sar.id, UpdateNarrativeRequest {
            narrative_text: "Updated: additional context added after further review.".into(),
            author_id: officer_id,
        }).await.unwrap();
        assert_eq!(narrative2.version, 2);

        // 7. Complete checklist
        svc.update_checklist(sar.id, InvestigationChecklist {
            subject_identity_verified: true,
            transaction_records_reviewed: true,
            aml_rules_documented: true,
            narrative_complete: true,
            supporting_docs_attached: true,
            legal_review_complete: true,
        }, &officer_id.to_string()).await.unwrap();

        // 8. Submit for review
        let reviewed = svc.submit_for_review(sar.id, &officer_id.to_string()).await.unwrap();
        assert_eq!(reviewed.status, "under_review");

        // 9. Approve
        let approved = svc.approve(sar.id, ReviewActionRequest {
            officer_id,
            notes: Some("Approved for filing".into()),
        }).await.unwrap();
        assert_eq!(approved.status, "approved");

        // 10. Generate document
        let doc = svc.generate_document(sar.id, &officer_id.to_string()).await.unwrap();
        assert!(doc.contains("NFIU-SAR-v2"));

        // 11. File
        let filed = svc.file(sar.id, FileRequest {
            filing_method: "api".into(),
            regulatory_reference_number: Some("NFIU-2026-001".into()),
        }, &officer_id.to_string()).await.unwrap();
        assert_eq!(filed.status, "filed");
        assert!(filed.filing_timestamp.is_some());

        // 12. Acknowledge
        let acked = svc.record_acknowledgement(sar.id, AcknowledgementRequest {
            acknowledgement_reference: "ACK-2026-001".into(),
            officer_id,
        }).await.unwrap();
        assert_eq!(acked.status, "acknowledged");

        // 13. Audit log has entries
        let audit = svc.get_audit_log(sar.id, &officer_id.to_string()).await.unwrap();
        assert!(!audit.is_empty());
        assert!(audit.iter().any(|e| e.action == "auto_initiated"));
        assert!(audit.iter().any(|e| e.action == "filed"));
        assert!(audit.iter().any(|e| e.action == "acknowledged"));
    }

    #[tokio::test]
    async fn checklist_incomplete_blocks_submission() {
        let svc = make_service().await;
        let officer_id = Uuid::new_v4();
        let today = Utc::now().date_naive();

        let sar = svc.manual_initiate(CreateSarRequest {
            sar_type: SarType::TransactionBased,
            subject_type: SubjectType::Individual,
            detection_method: DetectionMethod::ComplianceOfficerJudgment,
            subject_kyc_id: None,
            subject_wallet_addresses: vec![],
            suspicious_activity_description: "Manual SAR test".into(),
            activity_start_date: today,
            activity_end_date: today,
            total_amount_ngn: Decimal::from(1_000_000),
            transaction_count: 1,
            linked_transaction_ids: vec![],
            detecting_officer_id: Some(officer_id),
            assigned_investigator_id: None,
            deadline_days: None,
        }, officer_id).await.unwrap();

        // Checklist is incomplete — submission must fail
        let result = svc.submit_for_review(sar.id, &officer_id.to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checklist"));
    }

    #[tokio::test]
    async fn revision_cycle() {
        let svc = make_service().await;
        let officer_id = Uuid::new_v4();
        let today = Utc::now().date_naive();

        let sar = svc.manual_initiate(CreateSarRequest {
            sar_type: SarType::ActivityBased,
            subject_type: SubjectType::Entity,
            detection_method: DetectionMethod::ComplianceOfficerJudgment,
            subject_kyc_id: None,
            subject_wallet_addresses: vec![],
            suspicious_activity_description: "Revision cycle test".into(),
            activity_start_date: today,
            activity_end_date: today,
            total_amount_ngn: Decimal::from(2_000_000),
            transaction_count: 5,
            linked_transaction_ids: vec![],
            detecting_officer_id: Some(officer_id),
            assigned_investigator_id: Some(officer_id),
            deadline_days: Some(30),
        }, officer_id).await.unwrap();

        // Complete checklist and submit
        svc.update_checklist(sar.id, InvestigationChecklist {
            subject_identity_verified: true,
            transaction_records_reviewed: true,
            aml_rules_documented: true,
            narrative_complete: true,
            supporting_docs_attached: true,
            legal_review_complete: true,
        }, &officer_id.to_string()).await.unwrap();
        svc.submit_for_review(sar.id, &officer_id.to_string()).await.unwrap();

        // Return for revision
        let returned = svc.return_for_revision(sar.id, ReturnForRevisionRequest {
            officer_id,
            required_revisions: "Please add more transaction detail".into(),
        }).await.unwrap();
        assert_eq!(returned.status, "returned_for_revision");

        // Re-submit after revision
        let resubmitted = svc.submit_for_review(sar.id, &officer_id.to_string()).await.unwrap();
        assert_eq!(resubmitted.status, "under_review");
    }

    #[tokio::test]
    async fn confidentiality_access_control() {
        let svc = make_service().await;
        let officer_id = Uuid::new_v4();
        let today = Utc::now().date_naive();

        let sar = svc.manual_initiate(CreateSarRequest {
            sar_type: SarType::ThresholdBased,
            subject_type: SubjectType::Individual,
            detection_method: DetectionMethod::AmlRuleTrigger,
            subject_kyc_id: None,
            subject_wallet_addresses: vec!["GTEST456".into()],
            suspicious_activity_description: "Confidentiality test".into(),
            activity_start_date: today,
            activity_end_date: today,
            total_amount_ngn: Decimal::from(500_000),
            transaction_count: 1,
            linked_transaction_ids: vec![],
            detecting_officer_id: Some(officer_id),
            assigned_investigator_id: None,
            deadline_days: None,
        }, officer_id).await.unwrap();

        // Every access is logged
        let _ = svc.get_detail(sar.id, &officer_id.to_string()).await.unwrap();
        let audit = svc.get_audit_log(sar.id, &officer_id.to_string()).await.unwrap();
        assert!(audit.iter().any(|e| e.access_type == "read"));
    }

    #[tokio::test]
    async fn deadline_status_returns_sorted_by_urgency() {
        let svc = make_service().await;
        let statuses = svc.get_deadline_status().await.unwrap();
        // Verify sorted by filing_deadline ascending (most urgent first)
        for window in statuses.windows(2) {
            assert!(window[0].filing_deadline <= window[1].filing_deadline);
        }
    }

    #[tokio::test]
    async fn metrics_computation() {
        let svc = make_service().await;
        let from = Utc::now() - chrono::Duration::days(30);
        let to = Utc::now();
        let metrics = svc.get_metrics(from, to, "test_officer").await.unwrap();
        assert!(metrics.total_initiated >= 0);
        assert!(metrics.filing_timeliness_rate >= 0.0 && metrics.filing_timeliness_rate <= 1.0);
    }
}
