# Umbra Protocol

**Privacy-Preserving Enterprise Infrastructure for the Stellar Network**

Umbra is a suite of two confidentiality-focused enterprise tools built on Soroban (Stellar's smart contract platform). It lets regulated companies and B2B trading partners get the benefits of a public, auditable ledger — settlement finality, interoperability, programmability — without broadcasting sensitive financial data to every observer on that ledger.

> **Status:** Design / Early Development
> **Network:** Stellar (Soroban smart contracts)
> **License:** Apache 2.0 (see [LICENSE](#license))

---

## Table of Contents

- [Why Umbra](#why-umbra)
- [Modules](#modules)
  - [1. Umbra Audit — ZK-Proof Financial Audit Tool](#1-umbra-audit--zk-proof-financial-audit-tool)
  - [2. Umbra Escrow — Private B2B Supply Chain Escrow](#2-umbra-escrow--private-b2b-supply-chain-escrow)
- [Architecture](#architecture)
- [Repository Structure](#repository-structure)
- [Tech Stack](#tech-stack)
- [Getting Started](#getting-started)
- [Usage Examples](#usage-examples)
- [Security Model & Threat Considerations](#security-model--threat-considerations)
- [Terminology Note](#terminology-note)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Why Umbra

Public blockchains are transparent by design — every balance, transfer, and contract call is visible to anyone. That transparency is a feature for settlement integrity, but a liability for enterprises:

- A fintech proving solvency to a regulator doesn't want its treasury balance visible to competitors and counterparties.
- A manufacturer paying a supplier doesn't want rivals reverse-engineering its order volumes, unit pricing, or supplier list from on-chain payment flows.

Umbra's approach is **selective disclosure**: prove the *fact* a counterparty needs (solvency, payment completion, contract compliance) without revealing the *data* behind it (balances, amounts, identities). This is done via zero-knowledge proofs and encrypted/committed state, with Soroban contracts acting as the verifying and enforcing layer.

---

## Modules

### 1. Umbra Audit — ZK-Proof Financial Audit Tool

**Problem:** Regulated fintechs must periodically prove they hold sufficient capital/liquidity reserves. Traditional on-chain proof means exposing raw wallet balances — a competitive and security risk. Off-chain attestations, meanwhile, require the regulator to trust a third party instead of verifying cryptographically.

**Solution:** Umbra Audit lets a company generate a zero-knowledge proof that a private balance (or set of balances) satisfies a public threshold or formula — e.g., `reserves ≥ liabilities`, or `balance ≥ regulatory_minimum` — without revealing the balance itself. A Soroban contract verifies the proof on-chain, producing a publicly auditable, tamper-evident attestation that regulators (or auditors) can check at any time.

**Core properties:**

| Property | Description |
|---|---|
| **Confidentiality** | Actual balances, wallet composition, and transaction history remain private. |
| **Verifiability** | Anyone with contract access can verify the proof was validated on-chain — no trust in the company's self-reporting. |
| **Non-repudiation** | Once submitted, a proof is timestamped and immutable, creating an audit trail regulators can reference. |
| **Threshold flexibility** | Supports range proofs (`balance ≥ X`), ratio proofs (reserve/liability ratios), and multi-asset aggregation proofs. |
| **Regulator-defined parameters** | Reserve formulas and thresholds are set/updated by an authorized regulator role, not the company being audited. |

**High-level flow:**

1. Company computes actual reserve position off-chain (private).
2. Company's client generates a ZK proof (e.g., a Bulletproof-style range proof or a zk-SNARK circuit) attesting the position satisfies the regulator's published formula.
3. Proof + public inputs (threshold, timestamp, asset class) are submitted to the `umbra-audit` Soroban contract.
4. Contract verifies the proof on-chain and emits a `ProofVerified` event with a pass/fail result — never the underlying balance.
5. Regulator dashboard / API polls contract state for compliance status across all registered entities.

---

### 2. Umbra Escrow — Private B2B Supply Chain Escrow

**Problem:** High-value B2B payments on a public chain leak commercially sensitive metadata by default — payment amounts, frequency, and counterparty addresses let competitors infer supplier relationships, order volume, and negotiated pricing just by watching the chain.

**Solution:** Umbra Escrow is a Soroban-based escrow contract for supplier payments where the escrowed amount, and optionally counterparty identity, are shielded from public view while still being independently verifiable by the two parties (and Umbra itself, for dispute resolution) via commitments and ZK proofs.

**Core properties:**

| Property | Description |
|---|---|
| **Confidential amounts** | Payment amounts are stored as Pedersen commitments, not plaintext, on-chain. |
| **Conditional release** | Funds release automatically on milestone confirmation (e.g., delivery attestation, oracle signal, multi-sig approval) without revealing the release amount publicly. |
| **Dispute resolution** | A designated arbitration role can request selective decryption/disclosure of a specific escrow's details, scoped to that dispute only. |
| **Counterparty privacy (optional)** | Supports stealth-address-style payment addresses so recurring supplier relationships aren't trivially linkable across transactions. |
| **Auditability for participants** | Both buyer and supplier retain full plaintext visibility of their own transaction history at all times — privacy is from third-party observers, not from the counterparties themselves. |

**High-level flow:**

1. Buyer initiates escrow with a committed (hidden) amount and delivery/release conditions.
2. Supplier accepts; both parties hold the opening values for the commitment off-chain.
3. On milestone completion (delivery confirmation, oracle attestation, or manual multi-sig release), the contract verifies a ZK proof that the release matches the original commitment and releases funds.
4. Contract state publicly shows *that* an escrow was created, funded, and settled — but not the amount or, optionally, the counterparties.
5. In a dispute, the arbitration module can request a scoped disclosure proof from either party without exposing unrelated escrows.

---

## Architecture

```
                        ┌─────────────────────────────┐
                        │        Client Layer          │
                        │  (Company / Supplier / Reg.)  │
                        │  - Balance & proof generation │
                        │  - Wallet & key management    │
                        └───────────────┬───────────────┘
                                        │ ZK proof + public inputs
                                        ▼
        ┌───────────────────────────────────────────────────────┐
        │                    Soroban Contract Layer               │
        │  ┌─────────────────────┐   ┌──────────────────────┐    │
        │  │   umbra-audit.rs    │   │   umbra-escrow.rs     │    │
        │  │  - Proof verifier   │   │  - Commitment store    │    │
        │  │  - Threshold registry│   │  - Release conditions │    │
        │  │  - Event emitter    │   │  - Dispute hooks       │    │
        │  └─────────────────────┘   └──────────────────────┘    │
        │              shared: umbra-crypto (ZK verifier lib)      │
        └───────────────────────────┬───────────────────────────┘
                                    │ state reads / events
                                    ▼
                        ┌─────────────────────────────┐
                        │   Regulator / Dashboard API   │
                        │   - Compliance status feed    │
                        │   - Audit trail query          │
                        └─────────────────────────────┘
```

---

## Repository Structure

```
umbra-protocol/
├── contracts/
│   ├── umbra-audit/          # ZK financial audit Soroban contract
│   │   ├── src/lib.rs
│   │   ├── src/proof_verifier.rs
│   │   └── Cargo.toml
│   ├── umbra-escrow/         # Private B2B escrow Soroban contract
│   │   ├── src/lib.rs
│   │   ├── src/commitments.rs
│   │   ├── src/dispute.rs
│   │   └── Cargo.toml
│   └── umbra-crypto/         # Shared ZK proof generation/verification lib
│       ├── src/range_proof.rs
│       ├── src/commitment.rs
│       └── Cargo.toml
├── clients/
│   ├── audit-sdk/            # TypeScript SDK for proof generation (audit)
│   └── escrow-sdk/           # TypeScript SDK for commitment + escrow flows
├── dashboard/                 # Regulator/enterprise compliance dashboard (web)
├── scripts/
│   ├── deploy.sh
│   └── setup-local-network.sh
├── tests/
│   ├── audit_integration.rs
│   └── escrow_integration.rs
├── docs/
│   ├── threat-model.md
│   ├── proof-system.md
│   └── api-reference.md
├── .env.example
├── Cargo.toml                 # Workspace root
└── README.md
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Smart contracts | [Soroban](https://soroban.stellar.org/) (Rust) |
| Ledger | Stellar Network |
| ZK proof system | Bulletproofs / zk-SNARK circuit (range & equality proofs) — pluggable backend |
| Commitment scheme | Pedersen commitments |
| Client SDKs | TypeScript, `stellar-sdk` |
| Dashboard | React + TypeScript |
| Testing | Soroban CLI test harness, `cargo test` |

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain) + `wasm32-unknown-unknown` target
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)
- Node.js ≥ 18 and npm/yarn (for SDKs and dashboard)
- A funded Stellar testnet/futurenet account for local development

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/umbra-protocol.git
cd umbra-protocol

# Install Rust target for Soroban
rustup target add wasm32-unknown-unknown

# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Install SDK / dashboard dependencies
cd clients/audit-sdk && npm install && cd ../..
cd clients/escrow-sdk && npm install && cd ../..
cd dashboard && npm install && cd ..
```

### Local network setup

```bash
./scripts/setup-local-network.sh
```

### Deploy contracts to testnet

```bash
soroban contract deploy \
  --wasm contracts/umbra-audit/target/wasm32-unknown-unknown/release/umbra_audit.wasm \
  --source <your-identity> \
  --network testnet

soroban contract deploy \
  --wasm contracts/umbra-escrow/target/wasm32-unknown-unknown/release/umbra_escrow.wasm \
  --source <your-identity> \
  --network testnet
```

---

## Usage Examples

### Generating and submitting an audit proof (TypeScript SDK)

```typescript
import { AuditClient } from "@umbra/audit-sdk";

const client = new AuditClient({
  contractId: "CA...AUDIT_CONTRACT_ID",
  network: "testnet",
});

// Generate a ZK proof that reserves >= regulatory threshold
// without revealing the actual reserve amount
const proof = await client.generateReserveProof({
  actualReserves: privateReserveAmount, // never leaves this call
  threshold: publicThreshold,
  assetCode: "USDC",
});

const result = await client.submitProof(proof, {
  source: companyKeypair,
});

console.log("Compliance status:", result.verified); // true/false, no amount exposed
```

### Creating a confidential escrow (TypeScript SDK)

```typescript
import { EscrowClient } from "@umbra/escrow-sdk";

const client = new EscrowClient({
  contractId: "CA...ESCROW_CONTRACT_ID",
  network: "testnet",
});

const escrow = await client.createEscrow({
  supplier: supplierAddress,
  amount: 125_000_00, // hidden on-chain as a commitment
  releaseCondition: { type: "delivery-oracle", oracleId: "..." },
  source: buyerKeypair,
});

console.log("Escrow created:", escrow.id); // amount not visible on-chain
```

---

## Security Model & Threat Considerations

- **Proof soundness:** Both modules rely on the underlying ZK proof system being sound and the trusted setup (if a SNARK circuit is used) being conducted transparently. See [`docs/proof-system.md`](docs/proof-system.md) for the specific scheme and setup process.
- **Key management:** Loss of the private opening values for a commitment (Umbra Escrow) or the private balance witness (Umbra Audit) can lock funds or prevent proof regeneration. Enterprise deployments should pair Umbra with proper HSM/key-custody practices.
- **Regulator trust boundary:** Umbra Audit assumes the regulator role is honest in setting thresholds but does not need to be trusted with private balances. Threshold-setting should itself be governed (e.g., multi-sig or on-chain governance) to prevent unilateral manipulation.
- **Metadata leakage:** Even with hidden amounts, transaction timing and gas/fee patterns can leak partial information. Umbra does not fully solve traffic analysis and this should be treated as an open threat vector — see `docs/threat-model.md`.
- **Not audited:** This project has not undergone a third-party security audit. Do not use in production with real funds or real regulatory submissions until independently audited.

---

## Terminology Note

This README refers to the audit module's underlying confidentiality mechanism generically as a "ZK-proof" system. Some earlier project notes used the label **"Protocol 25 X-Ray"** as an internal/working codename for this mechanism. That name is **not** a verified, official Stellar protocol feature at the time of writing — it's treated here as a project-internal codename pending confirmation against official Stellar/Soroban documentation. If your team has a canonical spec for this, align `docs/proof-system.md` to it before external distribution.

---

## Roadmap

- [ ] Finalize ZK circuit design for Umbra Audit (range + ratio proofs)
- [ ] Implement Pedersen commitment module in `umbra-crypto`
- [ ] Umbra Escrow dispute/arbitration module
- [ ] Stealth-address support for supplier payment privacy
- [ ] Regulator dashboard MVP
- [ ] Independent security audit
- [ ] Mainnet deployment guide

---

## Contributing

Contributions are welcome. Please open an issue to discuss significant changes before submitting a pull request. All contract changes require accompanying tests in `tests/`.

---

## License

Apache 2.0. See `LICENSE` for details.
