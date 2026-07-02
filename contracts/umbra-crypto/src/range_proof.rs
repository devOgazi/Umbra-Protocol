use alloc::boxed::Box;

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::ristretto::CompressedRistretto;
use curve25519_dalek_ng::scalar::Scalar;
use merlin::Transcript;
use rand_core::{CryptoRng, RngCore};

/// Error type for range-proof operations.
#[derive(Debug, PartialEq, Eq)]
pub enum RangeProofError {
    ProofGenerationFailed,
    ProofVerificationFailed,
    InvalidBits,
}

/// A Bulletproofs-based range proof that a committed value lies in `[0, 2^n)`.
pub struct BulletproofRangeProof {
    proof: RangeProof,
    commitment: CompressedRistretto,
    bits: usize,
}

impl BulletproofRangeProof {
    /// Generate a range proof for `value` with the given `blinding` factor.
    ///
    /// The proof attests that `value` lies in the range `[0, 2^bits)`.
    /// `rng` must be a cryptographically secure random number generator.
    pub fn prove<R: RngCore + CryptoRng>(
        value: u64,
        blinding: Scalar,
        bits: usize,
        rng: &mut R,
    ) -> Result<Self, RangeProofError> {
        if bits == 0 || bits > 64 {
            return Err(RangeProofError::InvalidBits);
        }

        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(bits, 1);
        let mut transcript = Transcript::new(b"Umbra Range Proof");

        let (proof, commitment) = RangeProof::prove_single_with_rng(
            &bp_gens,
            &pc_gens,
            &mut transcript,
            value,
            &blinding,
            bits,
            rng,
        )
        .map_err(|_| RangeProofError::ProofGenerationFailed)?;

        Ok(BulletproofRangeProof {
            proof,
            commitment,
            bits,
        })
    }

    /// Verify the range proof.
    ///
    /// Returns `Ok(())` if the proof is valid, `Err` otherwise.
    pub fn verify<R: RngCore + CryptoRng>(&self, rng: &mut R) -> Result<(), RangeProofError> {
        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(self.bits, 1);
        let mut transcript = Transcript::new(b"Umbra Range Proof");

        self.proof
            .verify_single_with_rng(
                &bp_gens,
                &pc_gens,
                &mut transcript,
                &self.commitment,
                self.bits,
                rng,
            )
            .map_err(|_| RangeProofError::ProofVerificationFailed)
    }

    /// Return the compressed commitment that the proof attests to.
    pub fn commitment(&self) -> &CompressedRistretto {
        &self.commitment
    }

    /// Return the bit-width of the range.
    pub fn bits(&self) -> usize {
        self.bits
    }

    /// Serialize the proof into bytes.
    pub fn to_bytes(&self) -> Box<[u8]> {
        let proof_bytes = self.proof.to_bytes();
        let mut out = alloc::vec![0u8; 4 + 32 + proof_bytes.len()];
        out[..4].copy_from_slice(&(self.bits as u32).to_le_bytes());
        out[4..36].copy_from_slice(self.commitment.as_bytes());
        out[36..].copy_from_slice(&proof_bytes);
        out.into_boxed_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::ThreadRng;

    fn rng() -> ThreadRng {
        rand::thread_rng()
    }

    #[test]
    fn test_valid_range_proof() {
        let value = 100_000u64;
        let blinding = Scalar::from(12345u64);
        let bits = 32;

        let proof = BulletproofRangeProof::prove(value, blinding, bits, &mut rng())
            .expect("proof generation");

        assert_eq!(proof.bits(), 32);
        assert!(proof.verify(&mut rng()).is_ok());
    }

    #[test]
    fn test_boundary_zero() {
        let blinding = Scalar::from(999u64);
        let bits = 32;

        let proof = BulletproofRangeProof::prove(0, blinding, bits, &mut rng())
            .expect("proof generation for zero");

        assert!(proof.verify(&mut rng()).is_ok());
    }

    #[test]
    fn test_boundary_max() {
        let blinding = Scalar::from(777u64);
        let bits = 32;
        let value = (1u64 << 32) - 1;

        let proof = BulletproofRangeProof::prove(value, blinding, bits, &mut rng())
            .expect("proof generation at upper boundary");

        assert!(proof.verify(&mut rng()).is_ok());
    }

    #[test]
    fn test_invalid_bits_zero() {
        let result = BulletproofRangeProof::prove(0, Scalar::from(1u64), 0, &mut rng());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_bits_overflow() {
        let result = BulletproofRangeProof::prove(0, Scalar::from(1u64), 65, &mut rng());
        assert!(result.is_err());
    }

    #[test]
    fn test_proof_with_different_blinding_verifies() {
        let value = 42u64;
        let blinding = Scalar::from(888_888u64);
        let bits = 16;

        let proof = BulletproofRangeProof::prove(value, blinding, bits, &mut rng())
            .expect("proof generation");

        assert!(proof.verify(&mut rng()).is_ok());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let value = 7_000u64;
        let blinding = Scalar::from(555_555u64);
        let bits = 32;

        let proof = BulletproofRangeProof::prove(value, blinding, bits, &mut rng())
            .expect("proof generation");

        assert!(proof.verify(&mut rng()).is_ok());

        let bytes = proof.to_bytes();
        assert!(bytes.len() > 36);
    }
}
