#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    xdr::ToXdr,
};

pub mod proof_verifier;
use proof_verifier::{verify_attestation, VerificationResult};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Regulator,
    VerifierKey,
    /// Compliance threshold per asset, keyed by 4-byte asset code.
    Threshold(BytesN<4>),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted on every proof submission attempt (pass or fail).
///
/// # Privacy guarantee
/// Contains ONLY public data. The private balance is NEVER included here
/// or anywhere else in contract state, events, or return values.
/// See docs/proof-system.md.
#[contracttype]
#[derive(Clone)]
pub struct ProofVerifiedEvent {
    pub entity: Address,
    pub asset_code: BytesN<4>,
    pub threshold: u64,
    pub timestamp: u64,
    /// 32-byte Pedersen commitment — public, does not reveal the balance.
    pub commitment: BytesN<32>,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct UmbraAudit;

#[contractimpl]
impl UmbraAudit {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// Initialize the contract with a regulator address and verifier key.
    /// Must be called exactly once.
    pub fn init(env: Env, regulator: Address, verifier_key: BytesN<32>) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Regulator, &regulator);
        env.storage().instance().set(&DataKey::VerifierKey, &verifier_key);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.events().publish((symbol_short!("init"),), regulator);
    }

    // -----------------------------------------------------------------------
    // Threshold registry — regulator-only
    // -----------------------------------------------------------------------

    /// Set the compliance threshold for a given asset code.
    /// Only callable by the authorized regulator.
    pub fn set_threshold(env: Env, asset_code: BytesN<4>, threshold: u64) {
        let regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .expect("not initialized");
        regulator.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::Threshold(asset_code.clone()), &threshold);

        env.events()
            .publish((symbol_short!("set_thr"), asset_code), threshold);
    }

    /// Update the authorized verifier's Ed25519 public key.
    /// Only callable by the authorized regulator.
    pub fn set_verifier_key(env: Env, verifier_key: BytesN<32>) {
        let regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .expect("not initialized");
        regulator.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::VerifierKey, &verifier_key);

        env.events()
            .publish((symbol_short!("set_vk"),), verifier_key);
    }

    /// Read the current threshold for an asset code.
    pub fn get_threshold(env: Env, asset_code: BytesN<4>) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::Threshold(asset_code))
    }

    // -----------------------------------------------------------------------
    // Proof submission — single asset
    // -----------------------------------------------------------------------

    /// Submit an attested ZK proof result for a single asset.
    ///
    /// The client generates a Bulletproof off-chain proving `balance >= threshold`,
    /// an authorized verifier node checks it and signs the result, and this
    /// function verifies that Ed25519 signature on-chain.
    ///
    /// # Parameters
    /// - `entity`: the company submitting (must authorize the tx).
    /// - `asset_code`: 4-byte asset identifier.
    /// - `commitment`: 32-byte Pedersen commitment to the private balance.
    /// - `proof_result`: `1` = pass, `0` = fail (as attested by verifier).
    /// - `signature`: 64-byte Ed25519 signature from the authorized verifier.
    /// - `timestamp`: Unix timestamp for the audit trail.
    ///
    /// # Returns
    /// `true` if the attested proof passed, `false` if it failed.
    ///
    /// # Emits
    /// [`ProofVerifiedEvent`] — public data only, no balance exposed.
    ///
    /// # Panics
    /// If the Ed25519 signature is invalid (Soroban auth failure).
    pub fn submit_proof(
        env: Env,
        entity: Address,
        asset_code: BytesN<4>,
        commitment: BytesN<32>,
        proof_result: u32,
        signature: BytesN<64>,
        timestamp: u64,
    ) -> bool {
        entity.require_auth();

        let threshold: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold(asset_code.clone()))
            .expect("no threshold set for asset");

        let verifier_key: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized");

        let entity_bytes = entity_to_bytes(&env, &entity);

        let result = verify_attestation(
            &env,
            &verifier_key,
            &entity_bytes,
            &asset_code,
            threshold,
            timestamp,
            &commitment,
            proof_result as u8,
            &signature,
        );

        let passed = result == VerificationResult::Passed;

        // Emit audit event — no private data included.
        env.events().publish(
            (symbol_short!("pv"),),
            ProofVerifiedEvent {
                entity,
                asset_code,
                threshold,
                timestamp,
                commitment,
                passed,
            },
        );

        passed
    }

    // -----------------------------------------------------------------------
    // Proof submission — multi-asset aggregate
    // -----------------------------------------------------------------------

    /// Submit attested proofs for multiple assets and return aggregate pass/fail.
    ///
    /// Accepts a packed `Bytes` blob encoding N asset proofs. All per-asset
    /// proofs must pass for the aggregate result to be `true`. A
    /// [`ProofVerifiedEvent`] is emitted for each individual asset.
    ///
    /// # Encoding format
    /// The `proofs_blob` is a tightly packed sequence of fixed-size records.
    ///
    /// Layout: `4 + N * 108` bytes total.
    /// - Bytes 0..4: count as u32 little-endian
    /// - Bytes 4..: N records of 108 bytes each
    ///
    /// Each 108-byte record:
    /// - `[0..4]`    asset_code
    /// - `[4..36]`   commitment (32 bytes)
    /// - `[36]`      proof_result: 0x01=pass, 0x00=fail
    /// - `[37..40]`  reserved zero padding
    /// - `[40..104]` signature (64 bytes)
    /// - `[104..108]` reserved zero padding
    pub fn submit_proofs_multi(
        env: Env,
        entity: Address,
        proofs_blob: Bytes,
        timestamp: u64,
    ) -> bool {
        entity.require_auth();

        // proofs_blob layout:
        //   [0..4]   count: u32 LE
        //   [4..]    N records of 108 bytes each:
        //              [0..4]   asset_code
        //              [4..36]  commitment (32B)
        //              [36]     proof_result (0x01=pass, 0x00=fail)
        //              [37..40] reserved (zero)
        //              [40..104] signature (64B)
        //              [104..108] reserved (zero)

        if proofs_blob.len() < 4 {
            panic!("proofs_blob too short");
        }

        let count = u32::from_le_bytes([
            proofs_blob.get(0).unwrap(),
            proofs_blob.get(1).unwrap(),
            proofs_blob.get(2).unwrap(),
            proofs_blob.get(3).unwrap(),
        ]) as u32;

        const RECORD_SIZE: u32 = 108;
        if proofs_blob.len() != 4 + count * RECORD_SIZE {
            panic!("proofs_blob length mismatch");
        }

        let verifier_key: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized");

        let entity_bytes = entity_to_bytes(&env, &entity);
        let mut all_passed = true;

        for i in 0..count {
            let base = 4 + i * RECORD_SIZE;

            let asset_code: BytesN<4> = proofs_blob.slice(base..base + 4).try_into().unwrap();
            let commitment: BytesN<32> = proofs_blob.slice(base + 4..base + 36).try_into().unwrap();
            let proof_result: u8 = proofs_blob.get(base + 36).unwrap();
            let signature: BytesN<64> = proofs_blob.slice(base + 40..base + 104).try_into().unwrap();

            let threshold: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::Threshold(asset_code.clone()))
                .expect("no threshold set for asset");

            let result = verify_attestation(
                &env,
                &verifier_key,
                &entity_bytes,
                &asset_code,
                threshold,
                timestamp,
                &commitment,
                proof_result,
                &signature,
            );

            let passed = result == VerificationResult::Passed;
            if !passed {
                all_passed = false;
            }

            env.events().publish(
                (symbol_short!("pv"),),
                ProofVerifiedEvent {
                    entity: entity.clone(),
                    asset_code,
                    threshold,
                    timestamp,
                    commitment,
                    passed,
                },
            );
        }

        all_passed
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    pub fn get_regulator(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Regulator)
            .expect("not initialized")
    }

    pub fn get_verifier_key(env: Env) -> BytesN<32> {
        env.storage()
            .instance()
            .get(&DataKey::VerifierKey)
            .expect("not initialized")
    }

    pub fn version() -> u32 {
        2
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a Soroban `Address` to a deterministic 32-byte value for use in
/// the attestation message: SHA-256 of the address's strkey string bytes.
fn entity_to_bytes(env: &Env, address: &Address) -> BytesN<32> {
    // to_xdr() gives us the raw XDR bytes of the address — stable and canonical.
    let xdr = address.to_xdr(env);
    env.crypto().sha256(&xdr).into()
}
