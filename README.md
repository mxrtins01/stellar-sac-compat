# stellar-sac-compat

Stellar Asset Contract (SAC) vs. Custom WASM Dual-Asset Interoperability Harness.

## Overview

Soroban smart contracts and protocols frequently interact with two distinct classes of tokens:
1. **Stellar Asset Contract (SAC)**: Built-in contracts wrapping classic Stellar assets, with specific rules around trustlines, issuer clawbacks, asset authorization flags, and native precision.
2. **Custom WASM Tokens**: Standalone smart contracts implementing the SEP-41 token interface.

Subtle differences in behavior—such as trustline requirements, authorization flags, transfer error codes, and fee mechanics—often lead to unexpected contract failures when a protocol designed for custom tokens interacts with a SAC (or vice-versa).

`stellar-sac-compat` is a dual-target testing harness that runs contract interactions against both a native SAC instance and a standard WASM token simultaneously, pinpointing behavioral discrepancies and potential compatibility risks.

### Features

- **Dual-Execution Engine**: Runs the same test scenario concurrently against SAC and custom WASM targets.
- **Discrepancy Analyzer**: Highlights divergent behavior in return values, state mutations, error codes, and emitted events.
- **SAC Behavioral Scenarios**: Tests classic Stellar asset rules including:
  - Clawback operations and clawback-enabled trustlines
  - Issuer authorization flags (`AUTH_REQUIRED`, `AUTH_REVOCABLE`)
  - Trustline creation and balance limits
- **Interoperability Scorecard**: Generates a clear matrix showing compatibility and behavioral equivalence.

## Getting Started

### Prerequisites

- Rust 1.79+
- Soroban SDK

### Installation & Usage

```bash
# Run compatibility test matrix on a protocol contract
stellar-sac-compat run --contract path/to/protocol.wasm --scenarios all
```

## Roadmap

- [ ] Dual-target test execution runner
- [ ] SAC-specific feature suite (clawback, auth flags, trustlines)
- [ ] Error code and event divergence detector
- [ ] Comprehensive compatibility matrix reporter

## License

Apache-2.0 or MIT
