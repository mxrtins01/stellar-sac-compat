# Stellar SAC vs Custom WASM Compatibility Guide

`stellar-sac-compat` runs differential testing across Stellar Asset Contracts (SAC) and custom Soroban WASM tokens to detect operational divergences, gas disparities, and event mismatching.

---

## 1. Divergence Taxonomy & Severity

| Severity Level | Divergence Scenario | Impact |
|---|---|---|
| **High** | Authorization revocation / missing trustline divergence | Operations succeed on WASM but fail on SAC (or vice-versa), breaking protocol assumptions. |
| **High** | Clawback execution while flag is disabled | Unauthorized asset destruction or unexpected state changes. |
| **Medium** | Event signature or topic count divergence | Off-chain indexers and subgraphs fail to parse token events reliably. |
| **Low** | Gas / CPU instruction savings disparity | SAC executes with native host speed (~80%+ lower instructions vs WASM interpretation). |

---

## 2. CLI Runner Commands

```bash
# Execute differential test matrix across SAC and custom token
sac-compat run --sac <SAC_CONTRACT_ID> --wasm <WASM_CONTRACT_ID>

# Export compatibility matrix in JSON or Markdown format
sac-compat run --sac <SAC_ID> --wasm <WASM_ID> --format json > matrix.json
sac-compat run --sac <SAC_ID> --wasm <WASM_ID> --format markdown > matrix.md
```
