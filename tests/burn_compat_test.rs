use stellar_sac_compat::{DualHarness, SacFlags};

#[test]
fn test_burn_dual_execution_equivalence() {
    let mut harness = DualHarness::new(SacFlags::default());
    harness.set_balance("ALICE", 1_000, true, true);

    let report = harness.execute_burn("ALICE", 300);
    assert!(!report.has_divergence);
    assert!(report.sac_outcome.success);
    assert!(report.wasm_outcome.success);
    assert_eq!(report.sac_outcome.balance_transferred, 300);
    assert_eq!(report.wasm_outcome.balance_transferred, 300);
}
