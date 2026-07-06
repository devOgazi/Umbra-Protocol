# Umbra Protocol — Threat Model

> **Status:** Complete for Week 1 foundation. Covers the four threat categories
> flagged in the README's [Security Model & Threat Considerations](../README.md#security-model--threat-considerations).
> Future iterations should add empirical testing of side channels and a
> formal verification of the delegated-verification protocol.

---

## 1. Proof Soundness / Trusted Setup

### Threat

If the ZK proof system is unsound, a prover could convince the contract that a
false statement is true — e.g., that `balance ≥ threshold` when it is not.

### Current mitigations

- **Bulletproofs are transparent:** No trusted setup required. The
  `PedersenGens::default()` and `BulletproofGens::new()` generators are
  deterministic and publicly verifiable. There is no toxic waste to leak.
- **Delegated verification:** The Soroban contract does not run Bulletproofs
  verification directly (see [docs/proof-system.md](./proof-system.md) for the
  wasm32 limitation). Instead, an off-chain verifier node runs full Bulletproofs
  verification and signs the result. The contract checks the Ed25519 signature.
- **Key rotation:** The regulator can rotate the verifier key at any time via
  `set_verifier_key()` (regulator-only). Compromised verifier keys can be
  revoked.

### Residual risk

- **Verifier key compromise:** If an attacker obtains the verifier's Ed25519
  secret key, they can sign arbitrary attestations, making any proof appear
  valid on-chain. Mitigation: use a hardware security module (HSM) for the
  verifier key; consider M-of-N verifier multi-sig in a future upgrade.
- **Bulletproofs implementation bug:** A flaw in the `bulletproofs` crate
  (v4.x) could produce false positives. Mitigation: upstream dependency
  vigilance; independent verification nodes running different implementations.
- **Oracle AI/fuzzing:** An attacker could try to find commitment pairs that
  accidentally open to a false value. Mitigated by the computational binding
  property of Pedersen commitments over Ristretto255 (discrete-log hardness).

---

## 2. Key Management

### Threat

The entity's blinding factors and the verifier's Ed25519 key must be kept
confidential. Loss or compromise can lead to:

- Inability to generate new proofs (lost blinding factor) — **availability**.
- Forged attestations (compromised verifier key) — **integrity/soundness**.
- Commitment opening (exposed blinding factor) — **confidentiality**.

### Current mitigations

- **Blinding factors never on-chain:** The random blinding factor `r` used in
  Pedersen commitments `C = v·G + r·H` is generated client-side and never
  transmitted to any contract or third party.
- **Verifier key is the sole on-chain trust anchor:** The contract stores only
  the Ed25519 verifying key. The corresponding secret key is held off-chain by
  the verifier operator.
- **Admin-controlled key rotation:** `set_verifier_key()` allows the contract
  admin (regulator for audit, admin for escrow) to rotate the verifier key at
  any time.

### Residual risk

- **No HSM integration yet:** The client SDKs do not currently integrate with
  hardware security modules. Enterprise deployments should pair Umbra with
  appropriate key-custody infrastructure.
- **No key backup for blinding factors:** If a company loses its blinding factor
  `r`, it cannot re-generate a proof for the same commitment. The company must
  create a new commitment (new escrow or new proof submission) with a fresh
  blinding factor.
- **Verifier key compromise window:** Between key compromise and on-chain
  rotation, an attacker can forge attestations. Mitigation: monitor for
  unexpected `ProofVerified` events; implement automated key rotation with
  incident response.

---

## 3. Regulator Trust Boundary

### Threat

The regulator role in Umbra Audit sets compliance thresholds and controls the
verifier key. A malicious or compromised regulator could:

- Set thresholds arbitrarily high so no company can pass — **availability**.
- Set thresholds arbitrarily low so failing companies appear compliant —
  **integrity**.
- Replace the verifier key with one the regulator controls, then forge
  attestations — **soundness**.

### Current mitigations

- **Threshold transparency:** Every `set_threshold` call emits an on-chain event
  (`set_thr`) that any observer can see. Threshold manipulation is publicly
  detectable.
- **Verifier key transparency:** Every `set_verifier_key` call emits an on-chain
  event (`set_vk`). Key rotation history is public.
- **No private balance exposure:** The regulator never learns private balances.
  Even with full control of the contract, the Pedersen commitments remain
  hiding.
- **Arbitrator separation (escrow):** In Umbra Escrow, the arbitrator (dispute
  resolver) is a separate role from the admin, preventing unilateral disclosure
  of escrow amounts without a legitimate dispute.

### Residual risk

- **Regulator is a single point of trust:** A malicious regulator can make the
  contract unusable or meaningless. Mitigation: future governance via multi-sig
  or DAO-controlled regulator role.
- **No on-chain governance yet:** Threshold changes are instant — there is no
  timelock or challenge period. A future upgrade should add a governance delay
  for parameter changes.

---

## 4. Metadata / Traffic-Analysis Leakage

### Threat

Even with hidden amounts, the observable properties of contract interactions
can leak information:

- **Timing:** A company that submits proofs at regular intervals reveals its
  compliance audit cadence.
- **Frequency:** A buyer that creates escrows with the same supplier address
  reveals a recurring business relationship.
- **Fee patterns:** Gas/fee spikes may correlate with high-value escrows or
  multi-asset proof submissions.
- **Event ordering:** The sequence of `ProofVerifiedEvent`s in a block can be
  correlated with off-chain data (e.g., news about company financials) to
  narrow possible balance ranges.

### Current mitigations

- **No amounts in events:** Amounts never appear in any on-chain event.
  Only opaque commitments are visible.
- **No counterparty privacy (escrow):** Buyer and supplier addresses are stored
  in plaintext in `EscrowRecord`. The README lists stealth-address support as a
  roadmap item.
- **Identical event structure:** `ProofVerifiedEvent` has the same shape for
  pass and fail — the `passed` field is a boolean; no additional data is
  revealed about the underlying balance.

### Residual risk

- **Traffic analysis is NOT solved:** Umbra does not currently implement
  countermeasures such as:
  - Randomized delays / submission batching
  - Stealth addressing for counterparty privacy
  - Constant-time gas consumption per operation
  - Proof of "no submission" (to mask audit cadence)
- **This is an open threat vector.** See "Limitations and Future Work" in
  [docs/proof-system.md](./proof-system.md#limitations-and-future-work).
  Enterprise deployments should consider additional operational security
  measures (e.g., submitting through a proxy, batching proofs, using VPNs).

---

## Summary of Threat Categories

| Threat Category | Severity | Primary Mitigation | Residual Risk |
|---|---|---|---|
| Proof soundness | High | Bulletproofs transparency; delegated Ed25519 verification | Verifier key compromise |
| Key management | High | Blinding factors off-chain; verifier key rotation | No HSM integration yet |
| Regulator trust | Medium | On-chain event transparency; no balance exposure | Single-actor governance |
| Metadata leakage | Medium | No amounts in events; identical event shapes | Traffic analysis unsolved |

---

## File References

| File | Role |
|---|---|
| `contracts/umbra-audit/src/lib.rs` | Contract: threshold registry, event emission |
| `contracts/umbra-escrow/src/lib.rs` | Contract: escrow creation, release, dispute |
| `docs/proof-system.md` | Detailed cryptographic scheme description |
| `README.md` | High-level security considerations |
