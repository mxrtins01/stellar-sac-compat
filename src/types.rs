use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetFlavor {
    StellarAssetContract,
    CustomWasm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SacFlags {
    pub auth_required: bool,
    pub auth_revocable: bool,
    pub clawback_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTrustline {
    pub account: String,
    pub balance: i128,
    pub is_authorized: bool,
    pub has_trustline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub success: bool,
    pub error_code: Option<String>,
    pub balance_transferred: i128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceConsumption {
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasComparison {
    pub operation: String,
    pub sac_consumption: ResourceConsumption,
    pub wasm_consumption: ResourceConsumption,
    pub cpu_savings_pct: f64,
}
