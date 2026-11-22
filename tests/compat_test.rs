use stellar_sac_compat::{DualHarness, SacFlags};

#[test]
fn test_behavioral_equivalence_with_trustlines() {
    let flags = SacFlags::default();
    let mut harness = DualHarness::new(flags);

    // Both Alice and Bob have trustlines
    harness.set_balance("ALICE", 1_000, true, true);
    harness.set_balance("BOB", 200, true, true);

    let report = harness.execute_transfer("ALICE", "BOB", 300);
    assert!(!report.has_divergence);
    assert!(report.sac_outcome.success);
    assert!(report.wasm_outcome.success);
    assert_eq!(report.sac_outcome.balance_transferred, 300);
    assert_eq!(report.wasm_outcome.balance_transferred, 300);
}

#[test]
fn test_missing_trustline_divergence_detection() {
    let flags = SacFlags::default();
    let mut harness = DualHarness::new(flags);

    // Alice has balance & trustline; Bob has NO trustline
    harness.set_balance("ALICE", 1_000, true, true);
    harness.set_balance("BOB", 0, false, false); // NO TRUSTLINE!

    let report = harness.execute_transfer("ALICE", "BOB", 300);
    assert!(report.has_divergence);
    assert!(!report.sac_outcome.success);
    assert!(report.wasm_outcome.success); // WASM token transfers without classic trustline
    assert_eq!(report.sac_outcome.error_code.as_deref(), Some("MISSING_TRUSTLINE"));
    assert!(report.root_cause.unwrap().contains("classic Stellar constraint"));
}

#[test]
fn test_auth_revocation_divergence_detection() {
    let flags = SacFlags {
        auth_required: true,
        auth_revocable: true,
        clawback_enabled: false,
    };
    let mut harness = DualHarness::new(flags);

    // Bob has trustline but is un-authorized by issuer
    harness.set_balance("ALICE", 1_000, true, true);
    harness.set_balance("BOB", 100, true, false); // NOT AUTHORIZED!

    let report = harness.execute_transfer("ALICE", "BOB", 250);
    assert!(report.has_divergence);
    assert_eq!(report.sac_outcome.error_code.as_deref(), Some("AUTH_REVOKED"));
    assert!(report.wasm_outcome.success);
}

#[test]
fn test_clawback_divergence_when_enabled() {
    let flags = SacFlags {
        auth_required: false,
        auth_revocable: false,
        clawback_enabled: true,
    };
    let mut harness = DualHarness::new(flags);
    harness.set_balance("VICTIM", 1_000, true, true);

    let report = harness.execute_clawback("ISSUER", "VICTIM", 400);
    assert!(report.has_divergence);
    assert!(report.sac_outcome.success);
    assert_eq!(report.sac_outcome.balance_transferred, 400);
    assert!(!report.wasm_outcome.success);
    assert_eq!(report.wasm_outcome.error_code.as_deref(), Some("UNSUPPORTED_OPERATION"));
    assert!(report.root_cause.unwrap().contains("custom WASM token has no clawback"));
}

#[test]
fn test_clawback_rejected_when_flag_disabled() {
    let flags = SacFlags {
        auth_required: false,
        auth_revocable: false,
        clawback_enabled: false,
    };
    let mut harness = DualHarness::new(flags);
    harness.set_balance("USER", 500, true, true);

    let report = harness.execute_clawback("ISSUER", "USER", 100);
    assert!(!report.sac_outcome.success);
    assert_eq!(report.sac_outcome.error_code.as_deref(), Some("CLAWBACK_NOT_ENABLED"));
}

#[test]
fn test_mint_trustline_and_auth_divergence() {
    let flags = SacFlags {
        auth_required: true,
        auth_revocable: false,
        clawback_enabled: false,
    };
    let mut harness = DualHarness::new(flags);

    // Recipient has no trustline setup in SAC
    let report = harness.execute_mint("ISSUER", "NEW_USER", 500);
    assert!(report.has_divergence);
    assert!(!report.sac_outcome.success);
    assert_eq!(report.sac_outcome.error_code.as_deref(), Some("NO_TRUSTLINE"));
    assert!(report.wasm_outcome.success);
    assert_eq!(report.wasm_outcome.balance_transferred, 500);
}

#[test]
fn test_freeze_divergence() {
    let mut harness = DualHarness::new(SacFlags {
        auth_revocable: true,
        ..SacFlags::default()
    });
    harness.set_balance("BAD_ACTOR", 500, true, true);

    let report = harness.execute_freeze("BAD_ACTOR");
    assert!(report.has_divergence);
    assert!(report.sac_outcome.success);
    assert!(!report.wasm_outcome.success);
    assert_eq!(report.wasm_outcome.error_code.as_deref(), Some("UNSUPPORTED_FREEZE"));
}

#[test]
fn test_classic_asset_flag_permutations() {
    for auth_required in [false, true] {
        for auth_revocable in [false, true] {
            for clawback_enabled in [false, true] {
                let flags = SacFlags {
                    auth_required,
                    auth_revocable,
                    clawback_enabled,
                };
                let mut harness = DualHarness::new(flags);
                harness.set_balance("ALICE", 1_000, true, false);
                harness.set_balance("BOB", 1_000, true, true);

                let transfer = harness.execute_transfer("ALICE", "ALICE", 10);
                assert_eq!(
                    transfer.sac_outcome.success,
                    !auth_required,
                    "auth_required={auth_required}, auth_revocable={auth_revocable}, clawback_enabled={clawback_enabled}"
                );
                if auth_required {
                    assert_eq!(transfer.sac_outcome.error_code.as_deref(), Some("AUTH_REVOKED"));
                }

                let freeze = harness.execute_freeze("BOB");
                assert_eq!(
                    freeze.sac_outcome.success,
                    auth_revocable,
                    "auth_required={auth_required}, auth_revocable={auth_revocable}, clawback_enabled={clawback_enabled}"
                );
                if !auth_revocable {
                    assert_eq!(
                        freeze.sac_outcome.error_code.as_deref(),
                        Some("AUTHORIZATION_NOT_REVOCABLE")
                    );
                }
                assert_eq!(harness.sac_accounts["BOB"].is_authorized, !auth_revocable);

                let clawback = harness.execute_clawback("ISSUER", "ALICE", 10);
                assert_eq!(
                    clawback.sac_outcome.success,
                    clawback_enabled,
                    "auth_required={auth_required}, auth_revocable={auth_revocable}, clawback_enabled={clawback_enabled}"
                );
                if !clawback_enabled {
                    assert_eq!(
                        clawback.sac_outcome.error_code.as_deref(),
                        Some("CLAWBACK_NOT_ENABLED")
                    );
                }
            }
        }
    }
}

#[test]
fn test_matrix_reporter_markdown() {
    use stellar_sac_compat::MatrixReporter;
    let mut harness = DualHarness::new(SacFlags::default());
    harness.set_balance("ALICE", 1_000, true, true);
    let r1 = harness.execute_transfer("ALICE", "BOB", 100);
    let md = MatrixReporter::format_markdown(&[r1]);
    assert!(md.contains("Compatibility Matrix"));
    assert!(md.contains("transfer(ALICE -> BOB"));
}

#[test]
fn test_allowance_expiration_divergence() {
    use stellar_sac_compat::DivergenceSeverity;

    let flags = SacFlags {
        clawback_enabled: false,
        auth_required: false,
        auth_revocable: false,
    };
    let mut harness = DualHarness::new(flags);
    harness.set_balance("ALICE", 1_000, true, true);
    harness.set_balance("BOB", 0, true, true);

    let report = harness.execute_allowance_transfer("BOB", "ALICE", 200, 1000, 1500);
    assert!(report.has_divergence);
    assert!(!report.sac_outcome.success);
    assert_eq!(report.sac_outcome.error_code, Some("ALLOWANCE_EXPIRED".into()));
    assert!(report.wasm_outcome.success);
    assert_eq!(report.severity(), DivergenceSeverity::Critical);
}

#[test]
fn test_event_topic_comparison_helper() {
    use stellar_sac_compat::EventComparison;

    let sac_topics = vec!["transfer".into(), "G_ALICE".into(), "G_BOB".into()];
    let wasm_topics = vec!["transfer".into(), "G_ALICE".into(), "G_BOB".into()];
    let match_cmp = EventComparison::new("transfer", sac_topics.clone(), wasm_topics);
    assert!(match_cmp.matches);

    let custom_wasm_topics = vec!["custom_transfer_event".into()];
    let div_cmp = EventComparison::new("transfer", sac_topics, custom_wasm_topics);
    assert!(!div_cmp.matches);
}

#[test]
fn test_matrix_csv_format() {
    use stellar_sac_compat::reporter::MatrixReporter;
    use stellar_sac_compat::dual_runner::DivergenceReport;
    use stellar_sac_compat::types::ExecutionOutcome;

    let report = DivergenceReport {
        operation: "transfer".into(),
        sac_outcome: ExecutionOutcome {
            success: true,
            error_code: None,
            balance_transferred: 100,
        },
        wasm_outcome: ExecutionOutcome {
            success: false,
            error_code: Some("insufficient_balance".into()),
            balance_transferred: 0,
        },
        has_divergence: true,
        root_cause: Some("Balance divergence".into()),
    };

    let csv = MatrixReporter::format_csv(&[report]);
    assert!(csv.starts_with("operation,sac_status,wasm_status,has_divergence,root_cause\n"));
    assert!(csv.contains("\"transfer\",PASS,FAIL,true,\"Balance divergence\""));
}

#[test]
fn test_divergence_severity_display() {
    use stellar_sac_compat::types::DivergenceSeverity;

    assert_eq!(DivergenceSeverity::Info.to_string(), "INFO");
    assert_eq!(DivergenceSeverity::Warning.to_string(), "WARNING");
    assert_eq!(DivergenceSeverity::Critical.to_string(), "CRITICAL");
}

#[test]
fn test_mock_compat_matrix_fixture_deserialization() {
    let p = std::path::Path::new("examples/fixtures/mock_compat_matrix.json");
    assert!(p.exists());
    let raw = std::fs::read_to_string(p).expect("read matrix json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse json");
    let arr = json["test_cases"].as_array().expect("test_cases array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["operation"], "transfer_missing_trustline");
    assert_eq!(arr[0]["divergence_expected"], true);
}

#[test]
fn test_return_value_comparator() {
    use stellar_sac_compat::ReturnValueComparison;

    let eq_cmp = ReturnValueComparison::compare("transfer", &[1, 2, 3], &[1, 2, 3]);
    assert!(eq_cmp.is_equivalent);

    let div_cmp = ReturnValueComparison::compare("transfer", &[1, 2, 3], &[1, 2, 4]);
    assert!(!div_cmp.is_equivalent);
    assert_eq!(div_cmp.operation, "transfer");
}

#[test]
fn test_matrix_json_format_and_cli_guide() {
    use stellar_sac_compat::reporter::MatrixReporter;
    use stellar_sac_compat::dual_runner::DivergenceReport;
    use stellar_sac_compat::types::ExecutionOutcome;

    let report = DivergenceReport {
        operation: "burn".into(),
        sac_outcome: ExecutionOutcome {
            success: true,
            error_code: None,
            balance_transferred: 50,
        },
        wasm_outcome: ExecutionOutcome {
            success: true,
            error_code: None,
            balance_transferred: 50,
        },
        has_divergence: false,
        root_cause: None,
    };

    let json_str = MatrixReporter::format_json(&[report]);
    assert!(json_str.contains("\"operation\": \"burn\""));
    assert!(json_str.contains("\"has_divergence\": false"));

    let guide_p = std::path::Path::new("docs/CLI_GUIDE.md");
    assert!(guide_p.exists());
    let guide_content = std::fs::read_to_string(guide_p).expect("read guide");
    assert!(guide_content.contains("Stellar SAC vs Custom WASM Compatibility Guide"));
    assert!(guide_content.contains("Divergence Taxonomy & Severity"));
}

#[test]
fn test_zero_allowance_transfer_from_equivalence() {
    use stellar_sac_compat::AllowanceEquivalence;

    // Zero allowance, 0 requested -> Ok
    assert!(AllowanceEquivalence::verify(0, 0).is_ok());

    // Zero allowance, > 0 requested -> Err
    assert!(AllowanceEquivalence::verify(0, 100).is_err());

    // Sufficient allowance -> Ok
    assert!(AllowanceEquivalence::verify(500, 250).is_ok());
}

#[test]
fn test_console_colorized_diff() {
    use stellar_sac_compat::reporter::MatrixReporter;
    use stellar_sac_compat::dual_runner::DivergenceReport;
    use stellar_sac_compat::types::ExecutionOutcome;

    let reports = vec![
        DivergenceReport {
            operation: "transfer".into(),
            sac_outcome: ExecutionOutcome { success: true, error_code: None, balance_transferred: 10 },
            wasm_outcome: ExecutionOutcome { success: true, error_code: None, balance_transferred: 10 },
            has_divergence: false,
            root_cause: None,
        },
        DivergenceReport {
            operation: "clawback".into(),
            sac_outcome: ExecutionOutcome { success: false, error_code: Some("unauthorized".into()), balance_transferred: 0 },
            wasm_outcome: ExecutionOutcome { success: true, error_code: None, balance_transferred: 10 },
            has_divergence: true,
            root_cause: Some("Flag disabled on SAC".into()),
        },
    ];

    let diff = MatrixReporter::format_console_diff(&reports);
    assert!(diff.contains("\x1b[32m[EQUIVALENT] transfer\x1b[0m"));
    assert!(diff.contains("\x1b[31m[DIVERGENCE] clawback\x1b[0m"));
    assert!(diff.contains("Flag disabled on SAC"));
}

#[test]
fn test_batch_transfers_simulation() {
    use stellar_sac_compat::{DualHarness, SacFlags};

    let mut harness = DualHarness::new(SacFlags::default());
    harness.set_balance("ALICE", 1000, true, true);
    harness.set_balance("BOB", 0, true, true);

    let batch = vec![
        ("ALICE", "BOB", 100),
        ("ALICE", "BOB", 200),
    ];
    let reports = harness.execute_batch_transfers(&batch);
    assert_eq!(reports.len(), 2);
    assert!(!reports[0].has_divergence);
    assert!(!reports[1].has_divergence);
}


