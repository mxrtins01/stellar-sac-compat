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

impl DivergenceReport {
    pub fn severity(&self) -> crate::types::DivergenceSeverity {
        if !self.has_divergence {
            crate::types::DivergenceSeverity::Info
        } else if self.sac_outcome.success != self.wasm_outcome.success {
            crate::types::DivergenceSeverity::Critical
        } else {
            crate::types::DivergenceSeverity::Warning
        }
    }
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

    pub fn execute_clawback(&mut self, _issuer: &str, from: &str, amount: i128) -> DivergenceReport {
        // SAC Lane: Requires clawback_enabled flag on the asset
        let sac_outcome = if !self.sac_flags.clawback_enabled {
            ExecutionOutcome {
                success: false,
                error_code: Some("CLAWBACK_NOT_ENABLED".into()),
                balance_transferred: 0,
            }
        } else {
            let from_acc = match self.sac_accounts.get(from) {
                Some(a) => a,
                None => {
                    return DivergenceReport {
                        operation: format!("clawback(from={}, amount={})", from, amount),
                        sac_outcome: ExecutionOutcome { success: false, error_code: Some("FROM_NOT_FOUND".into()), balance_transferred: 0 },
                        wasm_outcome: ExecutionOutcome { success: false, error_code: Some("UNSUPPORTED_OPERATION".into()), balance_transferred: 0 },
                        has_divergence: true,
                        root_cause: Some("Account not found in SAC".into()),
                    };
                }
            };

            if from_acc.balance < amount {
                ExecutionOutcome {
                    success: false,
                    error_code: Some("INSUFFICIENT_BALANCE".into()),
                    balance_transferred: 0,
                }
            } else {
                self.sac_accounts.get_mut(from).unwrap().balance -= amount;
                ExecutionOutcome {
                    success: true,
                    error_code: None,
                    balance_transferred: amount,
                }
            }
        };

        // WASM Lane: Standard SEP-41 custom tokens lack issuer clawback capabilities
        let wasm_outcome = ExecutionOutcome {
            success: false,
            error_code: Some("UNSUPPORTED_OPERATION".into()),
            balance_transferred: 0,
        };

        let has_divergence = sac_outcome != wasm_outcome;
        let root_cause = if has_divergence && sac_outcome.success {
            Some("SAC allows issuer clawback when enabled, but custom WASM token has no clawback interface".to_string())
        } else if has_divergence && !sac_outcome.success {
            Some(format!("SAC clawback failed ({:?}) while WASM returns unsupported", sac_outcome.error_code))
        } else {
            None
        };

        DivergenceReport {
            operation: format!("clawback(from={}, amount={})", from, amount),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause,
        }
    }

    pub fn execute_mint(&mut self, _issuer: &str, to: &str, amount: i128) -> DivergenceReport {
        // SAC Lane: Recipient must have established trustline and be authorized if auth_required
        let sac_outcome = match self.sac_accounts.get(to) {
            None => ExecutionOutcome {
                success: false,
                error_code: Some("NO_TRUSTLINE".into()),
                balance_transferred: 0,
            },
            Some(to_acc) if !to_acc.has_trustline => ExecutionOutcome {
                success: false,
                error_code: Some("MISSING_TRUSTLINE".into()),
                balance_transferred: 0,
            },
            Some(to_acc) if self.sac_flags.auth_required && !to_acc.is_authorized => {
                ExecutionOutcome {
                    success: false,
                    error_code: Some("AUTH_REVOKED".into()),
                    balance_transferred: 0,
                }
            }
            Some(_) => {
                self.sac_accounts.get_mut(to).unwrap().balance += amount;
                ExecutionOutcome {
                    success: true,
                    error_code: None,
                    balance_transferred: amount,
                }
            }
        };

        // WASM Lane: Can mint to any address without pre-existing trustline
        *self.wasm_balances.entry(to.to_string()).or_insert(0) += amount;
        let wasm_outcome = ExecutionOutcome {
            success: true,
            error_code: None,
            balance_transferred: amount,
        };

        let has_divergence = sac_outcome != wasm_outcome;
        let root_cause = if has_divergence {
            Some(format!(
                "SAC mint requires classic trustline/authorization, while WASM allows unrestricted minting: {:?}",
                sac_outcome.error_code
            ))
        } else {
            None
        };

        DivergenceReport {
            operation: format!("mint(to={}, amount={})", to, amount),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause,
        }
    }

    pub fn execute_burn(&mut self, from: &str, amount: i128) -> DivergenceReport {
        // SAC Lane: Burning requires valid account and positive balance
        let sac_outcome = match self.sac_accounts.get_mut(from) {
            None => ExecutionOutcome {
                success: false,
                error_code: Some("ACCOUNT_NOT_FOUND".into()),
                balance_transferred: 0,
            },
            Some(acc) if acc.balance < amount => ExecutionOutcome {
                success: false,
                error_code: Some("INSUFFICIENT_BALANCE".into()),
                balance_transferred: 0,
            },
            Some(acc) => {
                acc.balance -= amount;
                ExecutionOutcome {
                    success: true,
                    error_code: None,
                    balance_transferred: amount,
                }
            }
        };

        // WASM Lane
        let wasm_outcome = match self.wasm_balances.get_mut(from) {
            None => ExecutionOutcome {
                success: false,
                error_code: Some("INSUFFICIENT_BALANCE".into()),
                balance_transferred: 0,
            },
            Some(bal) if *bal < amount => ExecutionOutcome {
                success: false,
                error_code: Some("INSUFFICIENT_BALANCE".into()),
                balance_transferred: 0,
            },
            Some(bal) => {
                *bal -= amount;
                ExecutionOutcome {
                    success: true,
                    error_code: None,
                    balance_transferred: amount,
                }
            }
        };

        let has_divergence = sac_outcome != wasm_outcome;
        DivergenceReport {
            operation: format!("burn(from={}, amount={})", from, amount),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause: if has_divergence {
                Some("Divergence between SAC and custom WASM burn semantics".into())
            } else {
                None
            },
        }
    }

    /// Freezes a classic Stellar trustline by revoking authorization.
    /// Custom WASM contracts do not expose issuer freeze hooks.
    pub fn execute_freeze(&mut self, account: &str) -> DivergenceReport {
        let sac_outcome = if !self.sac_flags.auth_revocable {
            ExecutionOutcome {
                success: false,
                error_code: Some("AUTHORIZATION_NOT_REVOCABLE".into()),
                balance_transferred: 0,
            }
        } else if let Some(acc) = self.sac_accounts.get_mut(account) {
            acc.is_authorized = false;
            ExecutionOutcome {
                success: true,
                error_code: None,
                balance_transferred: 0,
            }
        } else {
            ExecutionOutcome {
                success: false,
                error_code: Some("ACCOUNT_NOT_FOUND".into()),
                balance_transferred: 0,
            }
        };

        let wasm_outcome = ExecutionOutcome {
            success: false,
            error_code: Some("UNSUPPORTED_FREEZE".into()),
            balance_transferred: 0,
        };

        let has_divergence = sac_outcome != wasm_outcome;
        let root_cause = if !self.sac_flags.auth_revocable {
            "Classic SAC issuer authorization revocation is disabled by the asset flags; custom WASM has no freeze hook".into()
        } else {
            "Classic SAC supports issuer authorization revocation/freeze; custom WASM lacks native freeze hook".into()
        };
        DivergenceReport {
            operation: format!("freeze(account={})", account),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause: Some(root_cause),
        }
    }

    pub fn compare_resource_footprint(&self, operation: &str) -> crate::types::GasComparison {
        let (sac_cpu, wasm_cpu) = match operation {
            "transfer" => (18_500, 115_000),
            "burn" => (14_200, 95_000),
            _ => (22_000, 130_000),
        };

        let savings = ((wasm_cpu - sac_cpu) as f64) / (wasm_cpu as f64) * 100.0;

        crate::types::GasComparison {
            operation: operation.to_string(),
            sac_consumption: crate::types::ResourceConsumption {
                cpu_instructions: sac_cpu,
                memory_bytes: 4_200,
            },
            wasm_consumption: crate::types::ResourceConsumption {
                cpu_instructions: wasm_cpu,
                memory_bytes: 28_400,
            },
            cpu_savings_pct: savings,
        }
    }

    pub fn execute_allowance_transfer(
        &mut self,
        spender: &str,
        from: &str,
        amount: i128,
        expiration_ledger: u32,
        current_ledger: u32,
    ) -> DivergenceReport {
        let sac_outcome = if current_ledger > expiration_ledger {
            ExecutionOutcome {
                success: false,
                error_code: Some("ALLOWANCE_EXPIRED".into()),
                balance_transferred: 0,
            }
        } else {
            self.execute_sac_transfer(from, spender, amount)
        };

        let wasm_outcome = self.execute_wasm_transfer(from, spender, amount);
        let has_divergence = sac_outcome != wasm_outcome;
        let root_cause = if has_divergence {
            Some("SAC enforces ledger expiration boundaries on allowances, whereas naive WASM ignored expiration".into())
        } else {
            None
        };

        DivergenceReport {
            operation: format!("transfer_from(spender={}, from={}, amount={})", spender, from, amount),
            sac_outcome,
            wasm_outcome,
            has_divergence,
            root_cause,
        }
    }
}
