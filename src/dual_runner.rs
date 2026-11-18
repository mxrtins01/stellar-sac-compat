use crate::types::{AccountTrustline, ExecutionOutcome, SacFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceReport {
    pub operation: String,
    pub sac_outcome: ExecutionOutcome,
    pub wasm_outcome: ExecutionOutcome,
    pub has_divergence: bool,
    pub root_cause: Option<String>,
}

pub struct DualHarness {
    pub sac_flags: SacFlags,
    pub sac_accounts: HashMap<String, AccountTrustline>,
    pub wasm_balances: HashMap<String, i128>,
}

impl DualHarness {
    pub fn new(sac_flags: SacFlags) -> Self {
        Self {
            sac_flags,
            sac_accounts: HashMap::new(),
            wasm_balances: HashMap::new(),
        }
    }

    pub fn set_balance(&mut self, account: &str, amount: i128, has_trustline: bool, authorized: bool) {
        self.sac_accounts.insert(
            account.to_string(),
            AccountTrustline {
                account: account.to_string(),
                balance: amount,
                is_authorized: authorized,
                has_trustline,
            },
        );
        self.wasm_balances.insert(account.to_string(), amount);
    }

    pub fn execute_transfer(&mut self, from: &str, to: &str, amount: i128) -> DivergenceReport {
        // Lane A: SAC
        let sac_outcome = self.execute_sac_transfer(from, to, amount);

        // Lane B: WASM
        let wasm_outcome = self.execute_wasm_transfer(from, to, amount);

        let has_divergence = sac_outcome != wasm_outcome;
        let mut root_cause = None;

        if has_divergence {
            if !sac_outcome.success && wasm_outcome.success {
                if let Some(ref err) = sac_outcome.error_code {
                    root_cause = Some(format!("SAC transfer rejected due to classic Stellar constraint: {}", err));
                }
            } else if sac_outcome.success && !wasm_outcome.success {
                root_cause = Some("Custom WASM rejected while SAC succeeded".to_string());
            }
        }

        DivergenceReport {
            operation: format!("transfer({} -> {}, amount={})", from, to, amount),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause,
        }
    }

    fn execute_sac_transfer(&mut self, from: &str, to: &str, amount: i128) -> ExecutionOutcome {
        let from_acc = match self.sac_accounts.get(from) {
            Some(a) => a,
            None => return ExecutionOutcome { success: false, error_code: Some("FROM_NOT_FOUND".into()), balance_transferred: 0 },
        };

        if from_acc.balance < amount {
            return ExecutionOutcome { success: false, error_code: Some("INSUFFICIENT_BALANCE".into()), balance_transferred: 0 };
        }

        let to_acc = match self.sac_accounts.get(to) {
            Some(a) => a,
            None => return ExecutionOutcome { success: false, error_code: Some("NO_TRUSTLINE".into()), balance_transferred: 0 },
        };

        if !to_acc.has_trustline {
            return ExecutionOutcome { success: false, error_code: Some("MISSING_TRUSTLINE".into()), balance_transferred: 0 };
        }

        if self.sac_flags.auth_required && !to_acc.is_authorized {
            return ExecutionOutcome { success: false, error_code: Some("AUTH_REVOKED".into()), balance_transferred: 0 };
        }

        // Apply mutation
        self.sac_accounts.get_mut(from).unwrap().balance -= amount;
        self.sac_accounts.get_mut(to).unwrap().balance += amount;

        ExecutionOutcome { success: true, error_code: None, balance_transferred: amount }
    }

    fn execute_wasm_transfer(&mut self, from: &str, to: &str, amount: i128) -> ExecutionOutcome {
        let from_bal = self.wasm_balances.get(from).copied().unwrap_or(0);
        if from_bal < amount {
            return ExecutionOutcome { success: false, error_code: Some("INSUFFICIENT_BALANCE".into()), balance_transferred: 0 };
        }

        // WASM tokens only require existence or instantiate recipient balance on the fly
        *self.wasm_balances.entry(from.to_string()).or_insert(0) -= amount;
        *self.wasm_balances.entry(to.to_string()).or_insert(0) += amount;

        ExecutionOutcome { success: true, error_code: None, balance_transferred: amount }
    }
}
