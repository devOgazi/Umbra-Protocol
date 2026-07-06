# Contributing to Umbra Protocol

Thank you for your interest! Below is everything you need to get started.

---

## Workspace Layout

```
umbra-protocol/
├── contracts/
│   ├── umbra-audit/        # ZK financial audit Soroban contract
│   ├── umbra-escrow/       # Private B2B escrow Soroban contract
│   └── umbra-crypto/       # Shared ZK crypto library
├── clients/
│   ├── audit-sdk/          # TypeScript SDK for @umbra/audit-sdk
│   └── escrow-sdk/         # TypeScript SDK for @umbra/escrow-sdk
├── dashboard/              # Regulator/enterprise compliance dashboard (React)
├── scripts/                # Deployment and dev tooling
├── tests/                  # Integration test entry points
└── docs/                   # Documentation
```

---

## Prerequisites

- Rust (stable) + `wasm32-unknown-unknown` target
- Soroban CLI (see [setup guide](https://soroban.stellar.org/docs/getting-started/setup))
- Node.js ≥ 18 + npm/yarn
- A funded Stellar testnet account (for testnet deployment)

```bash
rustup target add wasm32-unknown-unknown
```

---

## Running Tests

### Full Rust workspace

```bash
cargo test --workspace
```

This runs unit and integration tests for all three crates:
- `umbra-crypto` — Pedersen commitment tests, range proof tests
- `umbra-audit` — contract integration tests (in-process mock ledger)
- `umbra-escrow` — contract integration tests (in-process mock ledger)

### TypeScript SDKs

```bash
# Audit SDK
cd clients/audit-sdk
npm install
npm test

# Escrow SDK
cd clients/escrow-sdk
npm install
npm test
```

### Cross-module end-to-end test

```bash
# Run against a local or testnet deployment:
# (see scripts/deploy.sh)
npm run test:e2e          # From the project root or SDK packages
```

### Full CI suite

```bash
# Run everything:
cargo test --workspace && \
  (cd clients/audit-sdk && npm test) && \
  (cd clients/escrow-sdk && npm test)
```

---

## Picking Up a Roadmap Item

See the [Roadmap section in README.md](./README.md#roadmap) for planned work.

Before starting on a Roadmap item:

1. **Open an issue** to discuss your approach (unless it's a trivial fix).
2. **Check existing PRs** to avoid duplicate work.
3. **Familiarise yourself** with the relevant `docs/` file:
   - ZK system changes → `docs/proof-system.md`
   - Threat model changes → `docs/threat-model.md`
   - API changes → `docs/api-reference.md`

### Current Roadmap priorities (Week 1 foundation complete)

| Item | Area | Notes |
|---|---|---|
| ZK circuit design | umbra-audit | Range + ratio proofs in umbra-crypto |
| Pedersen commitment module | umbra-crypto | Already implemented (tested) |
| Escrow dispute/arbitration | umbra-escrow | Already implemented (tested) |
| Stealth-address support | umbra-escrow | Not yet started |
| Regulator dashboard | dashboard | Scaffold exists, needs UX polish |
| Security audit | Project | Not yet done |
| Mainnet deployment guide | docs | Not yet written |

---

## Code Conventions

- **Rust contracts:** `#![no_std]`, Soroban SDK v21, no `std`-dependent crates
  in contract dependencies.
- **Privacy-first:** Never put a private amount/balance in an event, error
  message, log, or return value. The commitment is public; the underlying value
  is not.
- **TypeScript SDKs:** Use `stellar-sdk` for chain interaction — no alternative
  libraries.
- **Tests:** Every contract function should have at least one positive and one
  negative test. Add a privacy regression test for any function that handles
  committed/private values.

---

## Pull Request Process

1. All contract changes require accompanying tests in the relevant
   `contracts/*/tests/` directory.
2. Ensure `cargo test --workspace` passes.
3. If you change a contract interface, update the corresponding SDK and
   `docs/api-reference.md`.
4. If you change a confidentiality-critical path, add or update a privacy
   regression test.
5. Update `docs/CHANGELOG.md` with a one-line rationale for any API or
   behavioral change.

---

## Questions?

Open a [GitHub Discussion](https://github.com/your-org/umbra-protocol/discussions)
or join the [Stellar Developer Discord](https://discord.gg/stellar).
