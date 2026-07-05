//! Umbra Escrow — Private B2B Supply Chain Escrow
//!
//! Soroban contract entry points for the Umbra Escrow module. Wires together
//! the commitments and dispute sub-modules into the contract interface.
//!
//! ## Privacy guarantee
//!
//! Escrowed amounts are stored as Pedersen commitments (32 bytes, opaque).
//! The plaintext amount is NEVER stored on-chain, emitted in events, or
//! returned from any function except `request_disclosure` (arbitrator-gated).
//!
//! ## Stealth address note
//!
//! ROADMAP — NOT IMPLEMENTED: stealth-address-style counterparty privacy is
//! listed as a roadmap item in README §2. This contract does not implement
//! it. The buyer and supplier addresses are stored in plain text in the
//! EscrowRecord. A future version will add a stealth-address layer on top.

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env};

pub mod commitments;
pub mod dispute;

use commitments::{
    approve_multisig, cancel_escrow, create_escrow, get_escrow, next_escrow_id, EscrowRecord,
    ReleaseCondition,
};
use dispute::{get_arbitrator, request_disclosure, set_arbitrator};

// ---------------------------------------------------------------------------
// Storage keys (contract-level)
// ---------------------------------------------------------------------------

/// Contract-level storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract initialization guard.
    Initialized,
    /// Contract admin address.
    Admin,
    /// Ed25519 verifier public key (32B) for release attestations.
    VerifierKey,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct UmbraEscrow;

#[contractimpl]
impl UmbraEscrow {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// Initialize the contract.
    ///
    /// Must be called exactly once. Sets the admin, designates the arbitrator
    /// for dispute resolution, and registers the verifier's Ed25519 key used
    /// to validate oracle and multi-sig release attestations.
    ///
    /// # Parameters
    /// - `admin`: contract administrator (can rotate the verifier key).
    /// - `arbitrator`: address authorized to call `request_disclosure`.
    /// - `verifier_key`: 32-byte Ed25519 public key of the off-chain verifier
    ///   that attests release conditions.
    pub fn init(env: Env, admin: Address, arbitrator: Address, verifier_key: BytesN<32>) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::VerifierKey, &verifier_key);
        env.storage()
            .instance()
            .set(&DataKey::Initialized, &true);

        set_arbitrator(&env, arbitrator.clone());

        env.events()
            .publish((symbol_short!("init"),), (admin, arbitrator));
    }

    // -----------------------------------------------------------------------
    // Escrow creation
    // -----------------------------------------------------------------------

    /// Create a new escrow with a Pedersen-committed amount.
    ///
    /// The plaintext amount must NEVER be passed here. The caller commits
    /// to the amount off-chain and provides only the 32-byte Pedersen
    /// commitment.
    ///
    /// # Parameters
    /// - `buyer`: funder; must sign this transaction.
    /// - `supplier`: recipient on successful release.
    /// - `commitment`: 32-byte Pedersen commitment `C = v*G + r*H`.
    /// - `condition`: `DeliveryOracle(oracle_addr)` or
    ///   `MultiSig { required, signers }`.
    ///
    /// # Returns
    /// The newly assigned `escrow_id`.
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        supplier: Address,
        commitment: BytesN<32>,
        condition: ReleaseCondition,
    ) -> u64 {
        create_escrow(&env, buyer, supplier, commitment, condition)
    }

    // -----------------------------------------------------------------------
    // Release — DeliveryOracle path
    // -----------------------------------------------------------------------

    /// Release an escrow via delivery-oracle attestation.
    ///
    /// The designated oracle (set in the escrow's `condition`) must
    /// authorize this call and provide a valid Ed25519 release attestation.
    ///
    /// See `commitments::release_via_oracle` for the attestation format.
    ///
    /// # Panics
    /// - Escrow not found or not active.
    /// - Escrow condition is not DeliveryOracle.
    /// - Oracle auth fails.
    /// - Signature verification fails.
    pub fn release_oracle(env: Env, escrow_id: u64, signature: BytesN<64>) {
        let verifier_key: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized");

        commitments::release_via_oracle(&env, escrow_id, signature, verifier_key);
    }

    // -----------------------------------------------------------------------
    // Release — MultiSig path
    // -----------------------------------------------------------------------

    /// Submit a multi-sig approval for an escrow's release.
    ///
    /// Each call adds one approval from `approver`. When the total collected
    /// approvals reach the `required` threshold the escrow is released.
    ///
    /// Each approval includes a verifier-attested Ed25519 signature
    /// confirming the approver's intent and commitment validity.
    ///
    /// # Returns
    /// `true` if the escrow was released (threshold reached), `false` if
    /// more approvals are still needed.
    pub fn approve_multisig(
        env: Env,
        escrow_id: u64,
        approver: Address,
        signature: BytesN<64>,
    ) -> bool {
        let verifier_key: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized");

        approve_multisig(&env, escrow_id, approver, signature, verifier_key)
    }

    // -----------------------------------------------------------------------
    // Cancellation
    // -----------------------------------------------------------------------

    /// Cancel an active escrow. Only the buyer may cancel.
    pub fn cancel_escrow(env: Env, escrow_id: u64) {
        cancel_escrow(&env, escrow_id);
    }

    // -----------------------------------------------------------------------
    // Dispute / arbitration
    // -----------------------------------------------------------------------

    /// Disclose the committed value of a SINGLE escrow, arbitrator-gated.
    ///
    /// This is the ONLY path by which a committed value may be revealed
    /// on-chain. It is scoped exclusively to the named `escrow_id` — no
    /// other escrow's data can be accessed via this call.
    ///
    /// # Parameters
    /// - `escrow_id`: the specific escrow to disclose.
    /// - `value`: claimed plaintext amount to verify against the commitment.
    /// - `blinding_bytes`: 32-byte scalar blinding factor from escrow creation.
    ///
    /// # Returns
    /// The full `EscrowRecord` for the named escrow (commitment already
    /// public; value now disclosed via the dispute event).
    ///
    /// # Panics
    /// - Caller is not the designated arbitrator.
    /// - Escrow not found.
    /// - `(value, blinding_bytes)` do not open the stored commitment.
    pub fn request_disclosure(
        env: Env,
        escrow_id: u64,
        value: u64,
        blinding_bytes: BytesN<32>,
    ) -> EscrowRecord {
        request_disclosure(&env, escrow_id, value, blinding_bytes)
    }

    // -----------------------------------------------------------------------
    // Admin — verifier key rotation
    // -----------------------------------------------------------------------

    /// Update the verifier's Ed25519 public key. Admin-only.
    pub fn set_verifier_key(env: Env, verifier_key: BytesN<32>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::VerifierKey, &verifier_key);

        env.events()
            .publish((symbol_short!("set_vk"),), verifier_key);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Retrieve an escrow record. The commitment is public; the amount is not.
    pub fn get_escrow(env: Env, escrow_id: u64) -> EscrowRecord {
        get_escrow(&env, escrow_id)
    }

    /// Return the next escrow ID to be assigned.
    pub fn next_escrow_id(env: Env) -> u64 {
        next_escrow_id(&env)
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn get_arbitrator(env: Env) -> Address {
        get_arbitrator(&env)
    }

    pub fn get_verifier_key(env: Env) -> BytesN<32> {
        env.storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized")
    }

    pub fn version() -> u32 {
        1
    }
}

// ---------------------------------------------------------------------------
// MultiSig helper — expose Vec<Address> construction to callers
// ---------------------------------------------------------------------------
// Re-export ReleaseCondition so callers (tests, clients) can use it without
// reaching into the sub-module.
pub use commitments::{EscrowStatus, ReleaseCondition as EscrowReleaseCondition};
