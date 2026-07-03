/// Verify-only Bulletproofs range proof interface.
///
/// This module is gated behind the `verify` feature and does NOT require
/// `std` or any RNG — making it safe to use inside Soroban (wasm32) contracts.
/// Proof generation lives in `range_proof.rs` (requires `proofs` feature + std).
use alloc::vec::Vec;

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::ristretto::CompressedRistretto;
use merlin::Transcript;

/// Error variants for proof verification.
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The serialized proof bytes could not be deserialized.
    DeserializationFailed,
    /// The commitment bytes could not be deserialized.
    InvalidCommitment,
    /// The bit-width is outside the allowed range `[1, 64]`.
    InvalidBits,
    /// The proof did not verify against the commitment.
    ProofInvalid,
}

/// Verify a serialized Bulletproof range proof.
///
/// # Parameters
/// - `proof_bytes`: the raw proof bytes produced by
///   [`BulletproofRangeProof::to_bytes`] in `range_proof.rs`.
/// - `commitment_bytes`: 32-byte compressed Ristretto point (commitment).
/// - `bits`: bit-width of the claimed range `[0, 2^bits)`.
///
/// Returns `Ok(())` if the proof is valid, `Err(VerifyError)` otherwise.
///
/// # No private data
/// This function never takes or exposes the underlying value — only the
/// commitment (public) and proof bytes are processed.
pub fn verify_range_proof(
    proof_bytes: &[u8],
    commitment_bytes: &[u8; 32],
    bits: usize,
) -> Result<(), VerifyError> {
    if bits == 0 || bits > 64 {
        return Err(VerifyError::InvalidBits);
    }

    let commitment = CompressedRistretto(*commitment_bytes);

    let proof =
        RangeProof::from_bytes(proof_bytes).map_err(|_| VerifyError::DeserializationFailed)?;

    let pc_gens = PedersenGens::default();
    let bp_gens = BulletproofGens::new(bits, 1);
    let mut transcript = Transcript::new(b"Umbra Range Proof");

    proof
        .verify_single(&bp_gens, &pc_gens, &mut transcript, &commitment, bits)
        .map_err(|_| VerifyError::ProofInvalid)
}

/// Deserialize a proof + commitment bundle produced by
/// [`BulletproofRangeProof::to_bytes`].
///
/// Layout: `[ bits: u32 LE (4 bytes) | commitment: 32 bytes | proof bytes ]`
///
/// Returns `(proof_bytes, commitment_bytes, bits)` on success.
pub fn unpack_proof_bundle(bundle: &[u8]) -> Result<(Vec<u8>, [u8; 32], usize), VerifyError> {
    if bundle.len() < 36 {
        return Err(VerifyError::DeserializationFailed);
    }
    let bits = u32::from_le_bytes(bundle[..4].try_into().unwrap()) as usize;
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&bundle[4..36]);
    let proof_bytes = bundle[36..].to_vec();
    Ok((proof_bytes, commitment, bits))
}

/// Convenience wrapper: unpack and verify a proof bundle in one call.
///
/// # No private data
/// Only the commitment and proof bytes flow through this function.
pub fn verify_proof_bundle(bundle: &[u8]) -> Result<(), VerifyError> {
    let (proof_bytes, commitment_bytes, bits) = unpack_proof_bundle(bundle)?;
    verify_range_proof(&proof_bytes, &commitment_bytes, bits)
}
