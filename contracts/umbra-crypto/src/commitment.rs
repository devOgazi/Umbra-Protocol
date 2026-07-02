use curve25519_dalek_ng::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek_ng::scalar::Scalar;
use sha3::Sha3_512;

/// A Pedersen commitment: `C = v*G + r*H`
///
/// - `v` is the committed value
/// - `r` is a random blinding factor
/// - `G` and `H` are independent Ristretto generators
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commitment(CompressedRistretto);

/// Pre-computed independent generator `H` used in Pedersen commitments.
/// Derived deterministically from the basepoint so the scheme is transparent.
fn generator_h() -> RistrettoPoint {
    RistrettoPoint::hash_from_bytes::<Sha3_512>(b"Umbra Protocol Pedersen Commitment H")
}

impl Commitment {
    /// Create a new Pedersen commitment for `value` with the given `blinding` factor.
    pub fn new(value: u64, blinding: Scalar) -> Self {
        let h = generator_h();
        // commitment = value * G + blinding * H
        let point = RistrettoPoint::vartime_double_scalar_mul_basepoint(
            &blinding,
            &h,
            &Scalar::from(value),
        );
        Commitment(point.compress())
    }

    /// Verify that `value` and `blinding` open this commitment.
    pub fn open(&self, value: u64, blinding: Scalar) -> bool {
        &Commitment::new(value, blinding) == self
    }

    /// Return the compressed 32-byte representation.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Deserialize a commitment from its compressed form.
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        Some(Commitment(CompressedRistretto(*bytes)))
    }

    /// Equality check between two commitments.
    pub fn eq_commitment(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_blinding() -> Scalar {
        Scalar::from(42u64)
    }

    #[test]
    fn test_commit_and_open_correct() {
        let blinding = test_blinding();
        let comm = Commitment::new(100_000, blinding);
        assert!(comm.open(100_000, blinding));
    }

    #[test]
    fn test_commit_and_open_wrong_value() {
        let blinding = test_blinding();
        let comm = Commitment::new(100_000, blinding);
        assert!(!comm.open(99_999, blinding));
    }

    #[test]
    fn test_commit_and_open_wrong_blinding() {
        let comm = Commitment::new(100_000, Scalar::from(1u64));
        assert!(!comm.open(100_000, Scalar::from(2u64)));
    }

    #[test]
    fn test_equality_same_commitment() {
        let blinding = test_blinding();
        let a = Commitment::new(50, blinding);
        let b = Commitment::new(50, blinding);
        assert!(a.eq_commitment(&b));
    }

    #[test]
    fn test_equality_different_commitments() {
        let a = Commitment::new(50, Scalar::from(1u64));
        let b = Commitment::new(50, Scalar::from(2u64));
        assert!(!a.eq_commitment(&b));
    }

    #[test]
    fn test_roundtrip_bytes() {
        let blinding = test_blinding();
        let original = Commitment::new(7_000_000, blinding);
        let bytes = *original.as_bytes();
        let restored = Commitment::from_bytes(&bytes).unwrap();
        assert_eq!(original, restored);
        assert!(restored.open(7_000_000, blinding));
    }

    #[test]
    fn test_zero_value() {
        let blinding = test_blinding();
        let comm = Commitment::new(0, blinding);
        assert!(comm.open(0, blinding));
        assert!(!comm.open(1, blinding));
    }

    #[test]
    fn test_large_value() {
        let blinding = Scalar::from(999_999_999u64);
        let comm = Commitment::new(u64::MAX, blinding);
        assert!(comm.open(u64::MAX, blinding));
    }
}
