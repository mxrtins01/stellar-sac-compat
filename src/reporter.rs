use crate::dual_runner::DivergenceReport;

pub struct MatrixReporter;

impl MatrixReporter {
    pub fn format_markdown(reports: &[DivergenceReport]) -> String {
        let mut out = String::new();
        out.push_str("# Stellar SAC vs Custom WASM Compatibility Matrix\n\n");
        out.push_str("| Operation | SAC Status | WASM Status | Divergence? | Root Cause |\n");
        out.push_str("|---|:---:|:---:|:---:|---|\n");

        for r in reports {
            let sac_stat = if r.sac_outcome.success { "PASS" } else { "FAIL" };
            let wasm_stat = if r.wasm_outcome.success { "PASS" } else { "FAIL" };
            let div_str = if r.has_divergence { "⚠️ YES" } else { "✅ NO" };
            let cause = r.root_cause.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                r.operation, sac_stat, wasm_stat, div_str, cause
            ));
        }

        out
    }

    pub fn format_csv(reports: &[DivergenceReport]) -> String {
        let mut out = String::from("operation,sac_status,wasm_status,has_divergence,root_cause\n");
        for r in reports {
            let sac_stat = if r.sac_outcome.success { "PASS" } else { "FAIL" };
            let wasm_stat = if r.wasm_outcome.success { "PASS" } else { "FAIL" };
            let cause = r.root_cause.as_deref().unwrap_or("");
            out.push_str(&format!(
                "\"{}\",{},{},{},\"{}\"\n",
                r.operation, sac_stat, wasm_stat, r.has_divergence, cause
            ));
        }
        out
    }

    pub fn format_json(reports: &[DivergenceReport]) -> String {
        serde_json::to_string_pretty(reports).unwrap_or_else(|_| "[]".to_string())
    }
}
