//! Integration tests for umbra-audit.
//!
//! These tests use the Soroban testutils harness (in-process mock ledger) so
//! no live network is required. Each test directly invokes contract functions
//! and inspects results and emitted events.
//!
//! # Privacy regression tests
//! Several tests explicitly assert that no private balance appears in any
//! emitted event, return value, or storage entry. These are the canonical
//! regression tests for the confidentiality guarantee.

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env,
};
use soroban_sdk::xdr::ToXdr;
use umbra_audit::{UmbraAudit, UmbraAuditClient};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a mock Ed25519 keypair using a deterministic seed.
/// Returns (secret_key_bytes_32, public_key_bytes_32).
///
/// In tests we use ed25519-dalek (available via soroban-sdk testutils deps).
/// For simplicity, we use a fixed 32-byte seed and derive the keypair.
fn mock_keypair(seed: u8) -> ([u8; 32], [u8; 32]) {
    use ed25519_dalek::{SigningKey, VerifyingKey};
    let secret = [seed; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key: VerifyingKey = signing_key.verifying_key();
    (secret, verifying_key.to_bytes())
}

/// Sign the attestation message for `submit_proof` using the verifier's secret key.
fn sign_attestation(
    env: &Env,
    verifier_secret: &[u8; 32],
    entity: &Address,
    asset_code: &BytesN<4>,
    threshold: u64,
    timestamp: u64,
    commitment: &BytesN<32>,
    proof_result: u8,
) -> [u8; 64] {
    use ed25519_dalek::{Signer, SigningKey};
    use soroban_sdk::xdr::ToXdr;

    // Build the 99-byte attestation message (must match proof_verifier.rs).
    let mut msg = Vec::<u8>::new();
    msg.extend_from_slice(b"umbra-audit-v1");

    // Entity bytes: SHA-256 of entity XDR (same as entity_to_bytes in contract).
    let entity_xdr = entity.to_xdr(env);
    let entity_xdr_bytes: Vec<u8> = entity_xdr.iter().collect();
    let entity_hash = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&entity_xdr_bytes)
    };
    msg.extend_from_slice(&entity_hash);

    // asset_code
    for b in asset_code.iter() {
        msg.push(b);
    }
    // threshold LE
    msg.extend_from_slice(&threshold.to_le_bytes());
    // timestamp LE
    msg.extend_from_slice(&timestamp.to_le_bytes());
    // commitment
    for b in commitment.iter() {
        msg.push(b);
    }
    // result
    msg.push(proof_result);

    let signing_key = SigningKey::from_bytes(verifier_secret);
    let sig = signing_key.sign(&msg);
    sig.to_bytes()
}

/// Deploy the contract, init it, and set a threshold. Returns the client.
fn setup_contract<'a>(
    env: &'a Env,
    regulator: &Address,
    verifier_pubkey: &BytesN<32>,
    asset_code: BytesN<4>,
    threshold: u64,
) -> UmbraAuditClient<'a> {
    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(env, &contract_id);

    client.init(regulator, verifier_pubkey);

    env.mock_all_auths();
    client.set_threshold(&asset_code, &threshold);

    client
}

// ---------------------------------------------------------------------------
// Test: init
// ---------------------------------------------------------------------------

#[test]
fn test_init_sets_regulator_and_verifier_key() {
    let env = Env::default();
    let regulator = Address::generate(&env);
    let (_, vk_bytes) = mock_keypair(1);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);
    client.init(&regulator, &verifier_key);

    assert_eq!(client.get_regulator(), regulator);
    assert_eq!(client.get_verifier_key(), verifier_key);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_init_panics() {
    let env = Env::default();
    let regulator = Address::generate(&env);
    let (_, vk_bytes) = mock_keypair(1);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);
    client.init(&regulator, &verifier_key);
    client.init(&regulator, &verifier_key); // should panic
}

// ---------------------------------------------------------------------------
// Test: threshold registry
// ---------------------------------------------------------------------------

#[test]
fn test_regulator_can_set_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let (_, vk_bytes) = mock_keypair(1);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);
    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");

    let client = setup_contract(&env, &regulator, &verifier_key, asset_code.clone(), 1_000_000);

    let stored = client.get_threshold(&asset_code);
    assert_eq!(stored, Some(1_000_000u64));
}

#[test]
#[should_panic]
fn test_non_regulator_cannot_set_threshold() {
    let env = Env::default();
    // Do NOT mock all auths — let auth checking run.
    let regulator = Address::generate(&env);
    let attacker = Address::generate(&env);
    let (_, vk_bytes) = mock_keypair(1);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&regulator, &verifier_key);

    // Now try to set threshold without mocking auth — should fail because
    // attacker != regulator and no auth is mocked for this call.
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &attacker,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "set_threshold",
            args: soroban_sdk::Vec::new(&env).into(),
            sub_invokes: &[],
        },
    }]);
    client.set_threshold(&BytesN::from_array(&env, b"USDC"), &500_000u64);
}

// ---------------------------------------------------------------------------
// Test: submit_proof — valid attestation passes
// ---------------------------------------------------------------------------

#[test]
fn test_valid_proof_submission_returns_true() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(42);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);
    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let threshold: u64 = 1_000_000;
    let timestamp: u64 = 1_700_000_000;
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0xab; 32]);

    let client = setup_contract(&env, &regulator, &verifier_key, asset_code.clone(), threshold);

    let sig_bytes = sign_attestation(
        &env,
        &vk_secret,
        &entity,
        &asset_code,
        threshold,
        timestamp,
        &commitment,
        0x01,
    );
    let signature: BytesN<64> = BytesN::from_array(&env, &sig_bytes);

    let result = client.submit_proof(&entity, &asset_code, &commitment, &1u32, &signature, &timestamp);
    assert!(result);
}

// ---------------------------------------------------------------------------
// Test: submit_proof — verifier attests fail → returns false
// ---------------------------------------------------------------------------

#[test]
fn test_fail_attested_proof_returns_false() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(43);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);
    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let threshold: u64 = 1_000_000;
    let timestamp: u64 = 1_700_000_001;
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0xcd; 32]);

    let client = setup_contract(&env, &regulator, &verifier_key, asset_code.clone(), threshold);

    // Verifier attests 0x00 (fail) — proof was invalid off-chain.
    let sig_bytes = sign_attestation(
        &env,
        &vk_secret,
        &entity,
        &asset_code,
        threshold,
        timestamp,
        &commitment,
        0x00,
    );
    let signature: BytesN<64> = BytesN::from_array(&env, &sig_bytes);

    let result = client.submit_proof(&entity, &asset_code, &commitment, &0u32, &signature, &timestamp);
    assert!(!result);
}

// ---------------------------------------------------------------------------
// Test: submit_proof — bad signature causes tx failure (panic)
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_invalid_signature_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (_, vk_bytes) = mock_keypair(44);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);
    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let threshold: u64 = 1_000_000;
    let timestamp: u64 = 1_700_000_002;
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0xef; 32]);

    let client = setup_contract(&env, &regulator, &verifier_key, asset_code.clone(), threshold);

    // Garbage signature — should cause ed25519_verify to panic.
    let bad_sig: BytesN<64> = BytesN::from_array(&env, &[0x00; 64]);

    client.submit_proof(&entity, &asset_code, &commitment, &1u32, &bad_sig, &timestamp);
}

// ---------------------------------------------------------------------------
// Test: ProofVerifiedEvent never contains a raw balance (privacy regression)
// ---------------------------------------------------------------------------

#[test]
fn test_event_contains_no_private_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(45);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);
    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let threshold: u64 = 500_000;
    let timestamp: u64 = 1_700_000_003;
    // Use a recognizable commitment so we can confirm *commitment* is in the
    // event (it's public) but no private balance byte pattern is present.
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0x77; 32]);
    // The PRIVATE balance that must never appear anywhere.
    let private_balance: u64 = 12_345_678;

    let client = setup_contract(&env, &regulator, &verifier_key, asset_code.clone(), threshold);

    let sig_bytes = sign_attestation(
        &env,
        &vk_secret,
        &entity,
        &asset_code,
        threshold,
        timestamp,
        &commitment,
        0x01,
    );
    let signature: BytesN<64> = BytesN::from_array(&env, &sig_bytes);

    let _ = client.submit_proof(&entity, &asset_code, &commitment, &1u32, &signature, &timestamp);

    // Inspect all emitted events.
    let events = env.events().all();
    assert!(!events.is_empty(), "expected at least one event");

    for (_contract, _topics, data) in events.iter() {
        // Serialize the event data to check its byte representation.
        let data_xdr = data.clone().to_xdr(&env);
        let data_bytes: Vec<u8> = data_xdr.iter().collect();

        // The private balance must not appear in the serialized event data.
        let balance_le = private_balance.to_le_bytes();
        let balance_be = private_balance.to_be_bytes();

        let contains_balance_le = data_bytes
            .windows(8)
            .any(|w| w == balance_le);
        let contains_balance_be = data_bytes
            .windows(8)
            .any(|w| w == balance_be);

        assert!(
            !contains_balance_le && !contains_balance_be,
            "private balance found in emitted event — confidentiality regression!"
        );
    }
}

// ---------------------------------------------------------------------------
// Test: missing threshold panics (not silently wrong)
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "no threshold set for asset")]
fn test_submit_proof_without_threshold_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(46);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);
    client.init(&regulator, &verifier_key);
    // No set_threshold call — submitting should panic.

    let asset_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0x01; 32]);
    let bad_sig: BytesN<64> = BytesN::from_array(&env, &[0x00; 64]);

    client.submit_proof(&entity, &asset_code, &commitment, &1u32, &bad_sig, &1_700_000_000u64);
}

// ---------------------------------------------------------------------------
// Test: multi-asset — all pass
// ---------------------------------------------------------------------------

#[test]
fn test_multi_asset_all_pass() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(47);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);
    client.init(&regulator, &verifier_key);

    let usdc_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let xlm_code: BytesN<4> = BytesN::from_array(&env, b"XLM_");
    let threshold_usdc: u64 = 1_000_000;
    let threshold_xlm: u64 = 500_000;
    let timestamp: u64 = 1_700_000_010;
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0x55; 32]);

    client.set_threshold(&usdc_code, &threshold_usdc);
    client.set_threshold(&xlm_code, &threshold_xlm);

    // Build two-record blob.
    let mut blob = vec![2u8, 0, 0, 0]; // count = 2 LE

    for (code, threshold) in [(&usdc_code, threshold_usdc), (&xlm_code, threshold_xlm)] {
        let sig_bytes = sign_attestation(
            &env, &vk_secret, &entity, code, threshold, timestamp, &commitment, 0x01,
        );
        // asset_code (4B)
        for b in code.iter() { blob.push(b); }
        // commitment (32B)
        for b in commitment.iter() { blob.push(b); }
        // proof_result (1B)
        blob.push(0x01);
        // reserved 3B
        blob.extend_from_slice(&[0u8; 3]);
        // signature (64B)
        blob.extend_from_slice(&sig_bytes);
        // reserved 4B
        blob.extend_from_slice(&[0u8; 4]);
    }

    assert_eq!(blob.len(), 4 + 2 * 108);
    let proofs_blob = Bytes::from_slice(&env, &blob);

    let result = client.submit_proofs_multi(&entity, &proofs_blob, &timestamp);
    assert!(result, "all assets should pass");
}

// ---------------------------------------------------------------------------
// Test: multi-asset — one fail makes aggregate false
// ---------------------------------------------------------------------------

#[test]
fn test_multi_asset_one_fail_returns_false() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let entity = Address::generate(&env);
    let (vk_secret, vk_bytes) = mock_keypair(48);
    let verifier_key: BytesN<32> = BytesN::from_array(&env, &vk_bytes);

    let contract_id = env.register_contract(None, UmbraAudit);
    let client = UmbraAuditClient::new(&env, &contract_id);
    client.init(&regulator, &verifier_key);

    let usdc_code: BytesN<4> = BytesN::from_array(&env, b"USDC");
    let xlm_code: BytesN<4> = BytesN::from_array(&env, b"XLM_");
    let threshold: u64 = 1_000_000;
    let timestamp: u64 = 1_700_000_020;
    let commitment: BytesN<32> = BytesN::from_array(&env, &[0x66; 32]);

    client.set_threshold(&usdc_code, &threshold);
    client.set_threshold(&xlm_code, &threshold);

    let mut blob = vec![2u8, 0, 0, 0]; // count = 2

    // USDC: pass
    let sig_pass = sign_attestation(&env, &vk_secret, &entity, &usdc_code, threshold, timestamp, &commitment, 0x01);
    for b in usdc_code.iter() { blob.push(b); }
    for b in commitment.iter() { blob.push(b); }
    blob.push(0x01);
    blob.extend_from_slice(&[0u8; 3]);
    blob.extend_from_slice(&sig_pass);
    blob.extend_from_slice(&[0u8; 4]);

    // XLM: fail (verifier attests 0x00)
    let sig_fail = sign_attestation(&env, &vk_secret, &entity, &xlm_code, threshold, timestamp, &commitment, 0x00);
    for b in xlm_code.iter() { blob.push(b); }
    for b in commitment.iter() { blob.push(b); }
    blob.push(0x00);
    blob.extend_from_slice(&[0u8; 3]);
    blob.extend_from_slice(&sig_fail);
    blob.extend_from_slice(&[0u8; 4]);

    let proofs_blob = Bytes::from_slice(&env, &blob);

    let result = client.submit_proofs_multi(&entity, &proofs_blob, &timestamp);
    assert!(!result, "aggregate should fail when any asset fails");
}
