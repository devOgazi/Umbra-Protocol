//! Scoped arbitration disclosure for Umbra Escrow.
//!
//! ## Design
//!
//! The dispute module allows a designated arbitrator role to request
//! selective disclosure of a SINGLE escrow's committed value. The
//! disclosure is:
//!
//! - **Scoped**: only the escrow_id explicitly named in the call is
//!   disclosed. No other escrow's data is accessible via this path.
//! - **Proven**: the disclosing party provides a commitment opening
//!   `(value, blinding)` whose correctness is verified on-chain by
//!   recomputing the Pedersen commitment and comparing against what is
//!   stored for that specific escrow_id.
//! - **Arbitrator-gated**: only the address designated as arbitrator
//!   during contract initialization may call `request_disclosure`.
//!
//! ## Privacy guarantee
//!
//! This function discloses ONE escrow's value. The commitment stored for
//! every other escrow remains opaque — this call cannot, by construction,
//! return or leak any other escrow's opening values.
//!
//! ## Commitment opening verification
//!
//! We use umbra-crypto's `Commitment::from_bytes` + `Commitment::open` to
//! verify that `(value, blinding_bytes)` correctly opens the stored
//! commitment. If the opening is invalid the function panics.
//!
//! ## Disclosed event
//!
//! A `DisputeDisclosureEvent` is emitted containing the disclosed value
//! and which escrow it belongs to, so the event log is auditable.
//!
//! ## Stealth-address note
//!
//! ROADMAP — NOT IMPLEMENTED: selective counterparty address disclosure
//! (stealth-address unlinking) is listed as a future roadmap item in the
//! README. It is intentionally excluded from this module.

use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol};

use crate::commitments::{get_escrow, EscrowRecord};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

/// Storage keys specific to the dispute / arbitration module.
#[contracttype]
#[derive(Clone)]
pub enum DisputeKey {
    /// The designated arbitrator address.
    Arbitrator,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when the arbitrator requests and verifies a disclosure.
///
/// # Privacy note
/// This event intentionally reveals `value` for the specified `escrow_id`.
/// That is the point of the dispute-disclosure function. No other escrow's
/// data is included.
#[contracttype]
#[derive(Clone)]
pub struct DisputeDisclosureEvent {
    /// The escrow whose value was disclosed.
    pub escrow_id: u64,
    /// The arbitrator who requested the disclosure.
    pub arbitrator: Address,
    /// Disclosed plaintext amount (valid only for this escrow_id).
    pub value: u64,
    /// The commitment that was opened (matches contract storage for escrow_id).
    pub commitment: BytesN<32>,
}

// ---------------------------------------------------------------------------
// Arbitrator management
// ---------------------------------------------------------------------------

/// Set the designated arbitrator. Called once during contract initialization.
///
/// The arbitrator is the only address authorized to call
/// `request_disclosure`. Only the contract admin may set the arbitrator.
pub fn set_arbitrator(env: &Env, arbitrator: Address) {
    env.storage()
        .instance()
        .set(&DisputeKey::Arbitrator, &arbitrator);
}

/// Return the current arbitrator address.
pub fn get_arbitrator(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DisputeKey::Arbitrator)
        .expect("arbitrator not set")
}

// ---------------------------------------------------------------------------
// Scoped disclosure
// ---------------------------------------------------------------------------

/// Disclose the committed value of a single escrow, gated by the arbitrator.
///
/// This function is the only sanctioned path for revealing a committed
/// amount. Its scope is strictly limited to the `escrow_id` named in the
/// call.
///
/// # Parameters
/// - `escrow_id`: the escrow to disclose. Only this escrow is affected.
/// - `value`: the claimed plaintext amount that opens the commitment.
/// - `blinding_bytes`: the 32-byte scalar blinding factor used at creation.
///
/// # Returns
/// The `EscrowRecord` with the disclosed `value` verified against the
/// stored commitment.
///
/// # Panics
/// - If `arbitrator.require_auth()` fails (caller is not the arbitrator).
/// - If the escrow does not exist.
/// - If `(value, blinding_bytes)` do not correctly open the stored
///   commitment (commitment verification failure).
///
/// # Privacy guarantee
/// Only the commitment stored for `escrow_id` is checked. The function
/// neither reads nor can return data from any other escrow.
pub fn request_disclosure(
    env: &Env,
    escrow_id: u64,
    value: u64,
    blinding_bytes: BytesN<32>,
) -> EscrowRecord {
    // Only the arbitrator may call this function.
    let arbitrator: Address = env
        .storage()
        .instance()
        .get(&DisputeKey::Arbitrator)
        .expect("arbitrator not set");
    arbitrator.require_auth();

    // Fetch ONLY the requested escrow — no other escrow data is accessed.
    let record: EscrowRecord = get_escrow(env, escrow_id);

    // Verify the opening against the stored commitment.
    // Uses umbra-crypto Pedersen commitment: C = v*G + r*H.
    verify_commitment_opening(env, &record.commitment, value, &blinding_bytes);

    // Emit scoped disclosure event — arbitration trail.
    env.events().publish(
        (Symbol::new(env, "dispute_disclosed"),),
        DisputeDisclosureEvent {
            escrow_id,
            arbitrator,
            value,
            commitment: record.commitment.clone(),
        },
    );

    record
}

// ---------------------------------------------------------------------------
// Internal: commitment opening verification
// ---------------------------------------------------------------------------

/// Verify that `(value, blinding_bytes)` correctly opens `commitment`.
///
/// Recomputes `C' = value*G + blinding*H` and asserts `C' == commitment`.
/// Panics if the opening is invalid.
///
/// This is the on-chain proof of correct opening — no ZK proof needed
/// because the arbitrator is trusted and the opening is revealed to them.
/// The cryptographic check prevents the arbitrator from attributing a
/// false value to an escrow.
fn verify_commitment_opening(
    env: &Env,
    commitment: &BytesN<32>,
    value: u64,
    blinding_bytes: &BytesN<32>,
) {
    let blinding_arr: [u8; 32] = blinding_bytes.into();
    let commitment_arr: [u8; 32] = commitment.into();

    // Delegate to umbra-crypto's verify_opening_bytes, which handles the
    // curve25519_dalek_ng scalar construction internally. This avoids a direct
    // dependency on curve25519_dalek_ng from the contract crate.
    use umbra_crypto::commitment::Commitment;

    if !Commitment::verify_opening_bytes(&commitment_arr, value, &blinding_arr) {
        panic!("commitment opening verification failed: value and blinding do not match stored commitment");
    }

    // Suppress unused-env warning — env is needed for Soroban host functions
    // in callers of this function.
    let _ = env;
}
