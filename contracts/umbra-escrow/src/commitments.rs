//! Escrow commitment storage and release logic.
//!
//! ## Design overview
//!
//! Escrow amounts are stored as 32-byte Pedersen commitments (`C = v*G + r*H`)
//! rather than plaintext. The plaintext amount is NEVER written to contract
//! storage, emitted in events, or returned from any function.
//!
//! ## Release verification model
//!
//! Running Bulletproofs directly on wasm32-unknown-unknown is not feasible
//! (same constraint as umbra-audit). We use the same delegated attestation
//! model:
//!
//! 1. Off-chain: client generates a Pedersen commitment + opening values.
//! 2. Off-chain: an authorized verifier checks that the opening (value,
//!    blinding) correctly opens the stored commitment.
//! 3. The verifier signs the attestation with Ed25519.
//! 4. On-chain: this contract verifies the Ed25519 signature.
//!
//! Trust is in the verifier key, which is set by the contract admin.
//!
//! ## Release conditions (per README §2)
//!
//! - `DeliveryOracle` — a designated oracle signs delivery confirmation.
//! - `MultiSig` — M-of-N buyer+supplier multi-sig approvals.
//!
//! ## Attestation message format
//!
//! Release attestation (for oracle and multi-sig approvals) uses:
//!
//!   "umbra-escrow-release-v1" (23B)
//!   | escrow_id (8B LE)
//!   | commitment (32B)
//!   | result (1B: 0x01=release approved, 0x00=denied)
//!
//! Total: 64 bytes.
//!
//! ## Privacy guarantee
//!
//! Only `commitment_bytes` (32B, opaque) is stored on-chain. No amount, no
//! blinding factor is ever written to storage or events.

use soroban_sdk::{contracttype, Address, BytesN, Env, Symbol, Vec};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Parameters for the MultiSig release condition.
///
/// Stored as a struct (Soroban contracttype does not support named enum variant fields).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiSigParams {
    /// Number of approvals required (M-of-N).
    pub required: u32,
    /// List of valid signers (N addresses).
    pub signers: Vec<Address>,
}

/// The two release conditions supported by Umbra Escrow (per README §2).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseCondition {
    /// A single designated delivery oracle must sign release approval.
    DeliveryOracle(Address),
    /// M-of-N multi-signature approval.
    MultiSig(MultiSigParams),
}

/// Escrow status lifecycle.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscrowStatus {
    /// Escrow created and funded, awaiting release or dispute.
    Active,
    /// Funds released to supplier on successful condition verification.
    Released,
    /// Escrow cancelled or disputed; funds returned to buyer.
    Cancelled,
}

/// On-chain escrow record. The amount is NEVER stored — only its commitment.
///
/// # Privacy guarantee
/// `commitment` is a 32-byte Pedersen commitment `C = v*G + r*H`. The
/// underlying amount `v` and blinding factor `r` are known only to buyer
/// and supplier (held off-chain) and are never written to contract storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowRecord {
    /// Escrow unique identifier (monotonic counter).
    pub escrow_id: u64,
    /// Buyer (escrow creator / funder).
    pub buyer: Address,
    /// Supplier (funds recipient on release).
    pub supplier: Address,
    /// 32-byte Pedersen commitment to the escrow amount. Never the amount.
    pub commitment: BytesN<32>,
    /// Release condition: DeliveryOracle or MultiSig.
    pub condition: ReleaseCondition,
    /// Current lifecycle status.
    pub status: EscrowStatus,
    /// Ledger timestamp when this escrow was created.
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

/// Persistent storage keys for the escrow contract.
#[contracttype]
#[derive(Clone)]
pub enum EscrowKey {
    /// Next escrow ID to assign (monotonic u64 counter).
    NextId,
    /// EscrowRecord keyed by escrow_id.
    Record(u64),
    /// Number of MultiSig approvals received: keyed by (escrow_id, approver).
    Approval(u64, Address),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a new escrow is created.
///
/// # Privacy guarantee
/// Contains only the commitment (opaque 32B), never the plaintext amount.
#[contracttype]
#[derive(Clone)]
pub struct EscrowCreatedEvent {
    pub escrow_id: u64,
    pub buyer: Address,
    pub supplier: Address,
    /// 32-byte Pedersen commitment — does not reveal the amount.
    pub commitment: BytesN<32>,
    pub condition_type: Symbol,
}

/// Emitted when an escrow is released.
///
/// # Privacy guarantee
/// The released amount is not included. Only the commitment (already public
/// from EscrowCreatedEvent) and the verifier attestation are referenced.
#[contracttype]
#[derive(Clone)]
pub struct EscrowReleasedEvent {
    pub escrow_id: u64,
    pub supplier: Address,
    /// Same commitment as at creation — confirms which escrow was settled.
    pub commitment: BytesN<32>,
    /// Release path: "oracle" or "multisig".
    pub release_path: Symbol,
}

/// Emitted when an escrow is cancelled.
#[contracttype]
#[derive(Clone)]
pub struct EscrowCancelledEvent {
    pub escrow_id: u64,
    pub buyer: Address,
}

// ---------------------------------------------------------------------------
// Core escrow creation
// ---------------------------------------------------------------------------

/// Create a new escrow with a Pedersen-committed amount.
///
/// # Privacy guarantee
/// Only `commitment_bytes` is written to storage. The plaintext amount is
/// a client-side value and must never be passed to this function.
///
/// # Parameters
/// - `buyer`: must authorize this call.
/// - `supplier`: recipient if release conditions are met.
/// - `commitment_bytes`: 32-byte Pedersen commitment `C = v*G + r*H`.
/// - `condition`: `DeliveryOracle(oracle_addr)` or `MultiSig{required, signers}`.
///
/// # Returns
/// The newly assigned `escrow_id`.
pub fn create_escrow(
    env: &Env,
    buyer: Address,
    supplier: Address,
    commitment_bytes: BytesN<32>,
    condition: ReleaseCondition,
) -> u64 {
    buyer.require_auth();

    // Assign a monotonically increasing ID.
    let escrow_id: u64 = env
        .storage()
        .persistent()
        .get(&EscrowKey::NextId)
        .unwrap_or(0u64);

    let record = EscrowRecord {
        escrow_id,
        buyer: buyer.clone(),
        supplier: supplier.clone(),
        commitment: commitment_bytes.clone(),
        condition: condition.clone(),
        status: EscrowStatus::Active,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&EscrowKey::Record(escrow_id), &record);
    env.storage()
        .persistent()
        .set(&EscrowKey::NextId, &(escrow_id + 1));

    // Emit creation event — commitment (public), never the amount.
    let condition_type = match &condition {
        ReleaseCondition::DeliveryOracle(_) => Symbol::new(env, "oracle"),
        ReleaseCondition::MultiSig(_) => Symbol::new(env, "multisig"),
    };

    env.events().publish(
        (Symbol::new(env, "escrow_created"),),
        EscrowCreatedEvent {
            escrow_id,
            buyer,
            supplier,
            commitment: commitment_bytes,
            condition_type,
        },
    );

    escrow_id
}

// ---------------------------------------------------------------------------
// Release via DeliveryOracle attestation
// ---------------------------------------------------------------------------

/// Release an escrow via delivery-oracle attestation.
///
/// The oracle signs an attestation that the delivery condition is met and
/// that the opening of the commitment is valid. This function verifies the
/// Ed25519 signature on-chain.
///
/// # Attestation message
/// See module-level doc for the 64-byte message format.
///
/// # Parameters
/// - `escrow_id`: which escrow to release.
/// - `signature`: 64-byte Ed25519 signature from the oracle's key.
/// - `verifier_key`: the oracle/verifier's Ed25519 public key (32B, stored
///   in contract instance storage and passed in from lib.rs).
///
/// # Panics / errors
/// - Wrong escrow status.
/// - Caller is not the oracle designated in the release condition.
/// - Ed25519 signature invalid.
pub fn release_via_oracle(
    env: &Env,
    escrow_id: u64,
    signature: BytesN<64>,
    verifier_key: BytesN<32>,
) {
    let mut record: EscrowRecord = env
        .storage()
        .persistent()
        .get(&EscrowKey::Record(escrow_id))
        .expect("escrow not found");

    if record.status != EscrowStatus::Active {
        panic!("escrow not active");
    }

    // Confirm this escrow uses DeliveryOracle condition.
    let oracle_addr = match &record.condition {
        ReleaseCondition::DeliveryOracle(addr) => addr.clone(),
        _ => panic!("escrow condition is not DeliveryOracle"),
    };

    // Oracle must authorize this call.
    oracle_addr.require_auth();

    // Build attestation message and verify signature on-chain.
    verify_release_attestation(env, &verifier_key, escrow_id, &record.commitment, &signature);

    // Update record status.
    record.status = EscrowStatus::Released;
    env.storage()
        .persistent()
        .set(&EscrowKey::Record(escrow_id), &record);

    env.events().publish(
        (Symbol::new(env, "escrow_released"),),
        EscrowReleasedEvent {
            escrow_id,
            supplier: record.supplier,
            commitment: record.commitment,
            release_path: Symbol::new(env, "oracle"),
        },
    );
}

// ---------------------------------------------------------------------------
// Release via MultiSig approvals
// ---------------------------------------------------------------------------

/// Submit one multi-sig approval for an escrow's release.
///
/// Once `required` approvals are collected from valid signers, the escrow
/// status transitions to `Released`. Each signer's attestation signature
/// is verified on-chain.
///
/// # Parameters
/// - `escrow_id`: target escrow.
/// - `approver`: signer submitting this approval (must be in the signers list).
/// - `signature`: 64-byte Ed25519 attestation from the verifier confirming
///   this approver's consent and the commitment's validity.
/// - `verifier_key`: verifier's Ed25519 public key.
pub fn approve_multisig(
    env: &Env,
    escrow_id: u64,
    approver: Address,
    signature: BytesN<64>,
    verifier_key: BytesN<32>,
) -> bool {
    approver.require_auth();

    let mut record: EscrowRecord = env
        .storage()
        .persistent()
        .get(&EscrowKey::Record(escrow_id))
        .expect("escrow not found");

    if record.status != EscrowStatus::Active {
        panic!("escrow not active");
    }

    // Confirm MultiSig condition and that approver is a valid signer.
    let (required, signers) = match &record.condition {
        ReleaseCondition::MultiSig(params) => (params.required, params.signers.clone()),
        _ => panic!("escrow condition is not MultiSig"),
    };

    let is_valid_signer = signers.iter().any(|s| s == approver);
    if !is_valid_signer {
        panic!("approver is not in the signer list");
    }

    // Prevent double-approval.
    let approval_key = EscrowKey::Approval(escrow_id, approver.clone());
    if env.storage().persistent().has(&approval_key) {
        panic!("approver already submitted");
    }

    // Verify the attested signature before recording this approval.
    verify_release_attestation(env, &verifier_key, escrow_id, &record.commitment, &signature);

    // Record approval.
    env.storage().persistent().set(&approval_key, &true);

    // Count total approvals so far (scan signers list).
    let approval_count: u32 = signers
        .iter()
        .filter(|s| {
            env.storage()
                .persistent()
                .has(&EscrowKey::Approval(escrow_id, s.clone()))
        })
        .count() as u32;

    if approval_count >= required {
        // Threshold reached — release.
        record.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&EscrowKey::Record(escrow_id), &record);

        env.events().publish(
            (Symbol::new(env, "escrow_released"),),
            EscrowReleasedEvent {
                escrow_id,
                supplier: record.supplier,
                commitment: record.commitment,
                release_path: Symbol::new(env, "multisig"),
            },
        );

        return true; // Released
    }

    false // More approvals needed
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// Cancel an active escrow. Only the buyer can cancel, and only while Active.
pub fn cancel_escrow(env: &Env, escrow_id: u64) {
    let mut record: EscrowRecord = env
        .storage()
        .persistent()
        .get(&EscrowKey::Record(escrow_id))
        .expect("escrow not found");

    if record.status != EscrowStatus::Active {
        panic!("escrow not active");
    }

    record.buyer.require_auth();

    record.status = EscrowStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&EscrowKey::Record(escrow_id), &record);

    env.events().publish(
        (Symbol::new(env, "escrow_cancelled"),),
        EscrowCancelledEvent {
            escrow_id,
            buyer: record.buyer,
        },
    );
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Retrieve an escrow record by ID. Returns the full record including the
/// commitment bytes (public), but never a plaintext amount.
pub fn get_escrow(env: &Env, escrow_id: u64) -> EscrowRecord {
    env.storage()
        .persistent()
        .get(&EscrowKey::Record(escrow_id))
        .expect("escrow not found")
}

/// Return the next escrow ID that will be assigned.
pub fn next_escrow_id(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&EscrowKey::NextId)
        .unwrap_or(0u64)
}

// ---------------------------------------------------------------------------
// Internal: attestation verification
// ---------------------------------------------------------------------------

/// Build the release attestation message and verify the Ed25519 signature.
///
/// ## Message format (64 bytes)
///
///   "umbra-escrow-release-v1" (23B)
///   | escrow_id (8B LE)
///   | commitment (32B)
///   | result byte (1B: 0x01 = approved)
///
/// # Privacy guarantee
/// The message contains only public data (escrow ID + commitment). The
/// plaintext amount is never included.
fn verify_release_attestation(
    env: &Env,
    verifier_key: &BytesN<32>,
    escrow_id: u64,
    commitment: &BytesN<32>,
    signature: &BytesN<64>,
) {
    let mut msg = soroban_sdk::Bytes::new(env);

    // Prefix
    for &b in b"umbra-escrow-release-v1" {
        msg.push_back(b);
    }
    // escrow_id LE
    for b in escrow_id.to_le_bytes() {
        msg.push_back(b);
    }
    // commitment bytes
    for b in commitment.iter() {
        msg.push_back(b);
    }
    // result byte: 0x01 = approved
    msg.push_back(0x01);

    // Will panic (= tx failure) if signature is invalid.
    env.crypto().ed25519_verify(verifier_key, &msg, signature);
}
