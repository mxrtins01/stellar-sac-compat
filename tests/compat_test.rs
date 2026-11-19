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
