/// On-chain proof result verification for Umbra Audit.
///
/// ## Design rationale
///
/// Running a Bulletproofs verifier directly on Soroban (wasm32-unknown-unknown)
/// is not feasible: the `bulletproofs` crate's `clear_on_drop` dependency
/// requires a C compiler for wasm32. Instead, Umbra Audit uses a delegated
/// verification model:
///
/// 1. Client generates a Bulletproof range proof off-chain (umbra-crypto,
///    `proofs` feature).
/// 2. An authorized verifier node (run by or on behalf of the regulator)
///    runs full Bulletproof verification off-chain.
/// 3. The verifier signs the result with Ed25519.
/// 4. This contract checks the Ed25519 signature on-chain using Soroban's
///    native `env.crypto().ed25519_verify` host function.
///
/// Security model: trust is in the verifier's key, not its software.
/// The regulator sets and rotates the verifier key on-chain.
///
/// ## Attestation message format (99 bytes)
///
///   "umbra-audit-v1" (14B) | entity_address (32B) | asset_code (4B)
///   | threshold (8B LE) | timestamp (8B LE) | commitment (32B)
///   | result (1B: 0x01=pass, 0x00=fail)
///
/// ## Privacy guarantee
///
/// The private balance never appears on-chain. The commitment does not
/// reveal the underlying value.
use soroban_sdk::{BytesN, Env};

/// Outcome of an on-chain attestation verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationResult {
    /// Signature valid; attested result is pass.
    Passed,
    /// Signature valid; attested result is fail (Bulletproof was invalid).
    ProofFailed,
    /// Input data malformed (bad result byte, etc.).
    MalformedInput,
}

/// Verify the off-chain verifier's Ed25519 attestation on-chain.
///
/// If the signature is invalid, `ed25519_verify` panics (= tx failure),
/// which is the correct Soroban pattern for auth failures.
///
/// # Privacy guarantee
/// No private balance data is taken or returned by this function.
pub fn verify_attestation(
    env: &Env,
    verifier_pubkey: &BytesN<32>,
    entity_address: &BytesN<32>,
    asset_code: &BytesN<4>,
    threshold: u64,
    timestamp: u64,
    commitment: &BytesN<32>,
    proof_result: u8,
    signature: &BytesN<64>,
) -> VerificationResult {
    if proof_result != 0x00 && proof_result != 0x01 {
        return VerificationResult::MalformedInput;
    }

    // Build the 99-byte deterministic attestation message.
    // No private data included — only public inputs and boolean result.
    let mut msg = soroban_sdk::Bytes::new(env);
    for &b in b"umbra-audit-v1" {
        msg.push_back(b);
    }
    for b in entity_address.iter() {
        msg.push_back(b);
    }
    for b in asset_code.iter() {
        msg.push_back(b);
    }
    for b in threshold.to_le_bytes() {
        msg.push_back(b);
    }
    for b in timestamp.to_le_bytes() {
        msg.push_back(b);
    }
    for b in commitment.iter() {
        msg.push_back(b);
    }
    msg.push_back(proof_result);

    // Verify Ed25519. Panics on bad signature (tx failure) — correct pattern.
    env.crypto().ed25519_verify(verifier_pubkey, &msg, signature);

    if proof_result == 0x01 {
        VerificationResult::Passed
    } else {
        VerificationResult::ProofFailed
    }
}
