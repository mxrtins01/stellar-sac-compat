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


