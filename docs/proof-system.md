# Umbra Audit — Proof System

> **Status:** Canonical technical reference for the Umbra Audit ZK proof scheme.
>
> **Terminology note:** Earlier project notes used the label "Protocol 25 X-Ray"
> as an internal working codename for this mechanism. That name is not a
> verified, official Stellar protocol feature. This document supersedes all
> informal prior references. See also README.md § Terminology Note.

---

## Overview

Umbra Audit lets a regulated entity prove that a private financial balance
satisfies a regulator-defined threshold — without revealing the balance itself.
The on-chain contract produces a publicly auditable, tamper-evident attestation
that any observer can verify at any time.

The system has two cryptographic layers:

1. **Commitment layer** — Pedersen commitments hide the plaintext balance
   while binding the entity to a specific value.
2. **Range-proof layer** — Bulletproofs range proofs certify that the
   committed value lies in a range that implies `balance >= threshold`.

---

## Cryptographic Primitives

### Pedersen Commitments

A Pedersen commitment to value `v` with blinding factor `r` is:

```
C = v·G + r·H
```

where `G` is the Ristretto255 basepoint and `H` is a second independent
generator derived deterministically as:

```
H = hash_to_ristretto255("Umbra Protocol Pedersen Commitment H")
```

using SHA-3-512 as the hash function. Both generators are transparent and
publicly verifiable — there is no trusted setup for the commitment scheme.

**Properties:**
- **Hiding:** `C` reveals nothing about `v` to any observer who does not
  know `r`.
- **Binding:** It is computationally infeasible to find `(v', r')` with
  `v' ≠ v` such that `v'·G + r'·H = C`.

Implementation: `contracts/umbra-crypto/src/commitment.rs`

### Bulletproofs Range Proofs

Umbra uses the [Bulletproofs](https://eprint.iacr.org/2017/1066.pdf) protocol
to prove that a committed value lies in `[0, 2^n)` for a chosen bit-width `n`.

To prove `balance >= threshold`, the client proves that the committed value
`balance - threshold` lies in `[0, 2^n)` — which is equivalent to
`balance >= threshold` when the subtraction does not underflow.

**Properties:**
- **No trusted setup:** Bulletproofs are transparent (no CRS or toxic waste).
- **Logarithmic proof size:** A 32-bit range proof is ~700 bytes.
- **Soundness:** Under the discrete-log assumption over Ristretto255.

Transcript label (must match between prover and verifier): `b"Umbra Range Proof"`

Implementation: `contracts/umbra-crypto/src/range_proof.rs` (off-chain,
`proofs` feature required).

---

## On-Chain Verification Model

Soroban (wasm32-unknown-unknown) cannot run Bulletproofs verification
directly: the `bulletproofs` crate's `clear_on_drop` dependency requires a
C compiler for the wasm32 target, making it incompatible with the Soroban
build environment as of SDK v21.

Instead, Umbra Audit uses a **delegated verification** model:

```
Client                  Verifier Node          Soroban Contract
  │                          │                       │
  │── Bulletproof + inputs ──▶│                       │
  │                          │── verify off-chain     │
  │                          │── sign result ─────────▶│
  │                          │                       │── check Ed25519 sig
  │                          │                       │── emit ProofVerifiedEvent
  │◀─────────────────────────────────────────────────│
```

1. The client generates a Bulletproof range proof off-chain using `umbra-crypto`
   with the `proofs` feature.
2. The client submits the proof to an authorized **verifier node** — a
   service operated by or on behalf of the regulator.
3. The verifier node checks the proof with the full Bulletproof verifier
   and signs the result with its Ed25519 key.
4. The client submits the signed attestation to the Soroban contract.
5. The contract verifies the Ed25519 signature on-chain using Soroban's
   native `env.crypto().ed25519_verify` host function.
6. The contract emits a `ProofVerifiedEvent` with the public result.

**Trust model:** Trust is in the verifier's Ed25519 key, not its software.
The regulator controls the verifier key on-chain (via `set_verifier_key`,
which requires regulator auth). Multiple independent verifier nodes can be
run; the contract can be upgraded to require M-of-N verifier signatures for
higher assurance.

---

## Attestation Message Format

The verifier signs the following 99-byte message (Ed25519, SHA-512 internally):

```
"umbra-audit-v1"  (14 bytes, ASCII)
entity_address    (32 bytes, SHA-256 of entity XDR)
asset_code        (4 bytes)
threshold         (8 bytes, little-endian u64)
timestamp         (8 bytes, little-endian u64)
commitment        (32 bytes, compressed Ristretto point)
result            (1 byte: 0x01 = pass, 0x00 = fail)
```

The entity address is encoded as `SHA-256(entity.to_xdr(env))` to produce a
stable 32-byte value that is consistent between the off-chain verifier
(which has access to the same XDR serialization) and the on-chain contract.

---

## Privacy Guarantee

The private balance **never appears** in:

- Contract storage (only the commitment is stored, via the `ProofVerifiedEvent`)
- Emitted events (`ProofVerifiedEvent` contains only public fields)
- Return values of any contract function
- Error messages or panic strings
- The attestation message (only the commitment — a public cryptographic object)

The commitment `C = v·G + r·H` is a public object. Knowing `C` gives no
information about `v` to any party who does not know the blinding factor `r`.
The blinding factor is generated by the client and never transmitted on-chain.

The integration test `test_event_contains_no_private_balance` is the
canonical regression test for this guarantee. It serializes every emitted
event to XDR and asserts that the private balance byte pattern does not appear.

---

## Key Management Considerations

- **Entity (client):** Must safeguard the blinding factor `r`. Loss of `r`
  prevents future proof generation for the same commitment. Enterprise
  deployments should use HSM-backed key custody for blinding factors.
- **Verifier node:** The Ed25519 signing key must be kept confidential. Key
  rotation is possible via `set_verifier_key` (regulator auth required).
- **Regulator:** The regulator address should be a multi-sig or governance
  contract to prevent unilateral threshold or key manipulation.

---

## Limitations and Future Work

- **Verifier liveness:** The delegated model introduces a liveness dependency
  on the verifier node. If the verifier is unavailable, proof submission stalls.
  Future work: direct on-chain Bulletproof verification once the wasm32
  compatibility issue in `bulletproofs`/`clear_on_drop` is resolved upstream.
- **Verifier honesty:** A malicious verifier could sign a false attestation.
  Mitigations: multiple independent verifiers, verifier key transparency,
  on-chain verifier key history log.
- **Metadata leakage:** Transaction timing, fee patterns, and submission
  frequency can leak partial information even when amounts are hidden.
  See `docs/threat-model.md`.
- **Not audited:** This implementation has not undergone a third-party security
  audit. Do not use in production until independently audited.

---

## File References

| File | Role |
|---|---|
| `contracts/umbra-crypto/src/commitment.rs` | Pedersen commitment implementation |
| `contracts/umbra-crypto/src/range_proof.rs` | Bulletproofs range proof (off-chain, `proofs` feature) |
| `contracts/umbra-audit/src/proof_verifier.rs` | On-chain attestation verification |
| `contracts/umbra-audit/src/lib.rs` | Contract: threshold registry, event emission |
| `contracts/umbra-audit/tests/audit_integration.rs` | Integration tests including privacy regression tests |
