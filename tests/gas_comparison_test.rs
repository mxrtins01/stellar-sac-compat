use stellar_sac_compat::{DualHarness, SacFlags};

#[test]
fn test_gas_optimization_advantage_of_sac() {
    let harness = DualHarness::new(SacFlags::default());
    let gas = harness.compare_resource_footprint("transfer");

    assert!(gas.sac_consumption.cpu_instructions < gas.wasm_consumption.cpu_instructions);
    assert!(gas.cpu_savings_pct > 80.0, "SAC should provide >80% CPU instructions savings over custom WASM");
}
