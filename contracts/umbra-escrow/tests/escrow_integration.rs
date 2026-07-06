//! Integration tests for umbra-escrow.
//!
//! Uses the Soroban testutils harness (in-process mock ledger). No live
//! network required.
//!
//! # Privacy regression tests
//! Tests explicitly assert that no escrow amount appears in events, return
//! values, or storage beyond the opaque commitment bytes.

#![cfg(test)]

use soroban_sdk::{
    contracttype,
    testutils::{Address as _, Events},
    Address, BytesN, Env, Symbol, Vec,
};
use soroban_sdk::xdr::ToXdr;
use umbra_escrow::{
    UmbraEscrow, UmbraEscrowClient,
    commitments::{ReleaseCondition, MultiSigParams, EscrowStatus, EscrowCreatedEvent, EscrowReleasedEvent, EscrowCancelledEvent},
    dispute::DisputeDisclosureEvent,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic "blinding factor" used across tests.
fn test_blinding() -> [u8; 32] {
    let mut b = [0u8; 32];
    b[0..8].copy_from_slice(&42u64.to_le_bytes());
    b
}

/// Build a Pedersen commitment from `value` using umbra-crypto.
fn make_commitment(value: u64) -> [u8; 32] {
    use umbra_crypto::commitment::Commitment;
    use curve25519_dalek_ng::scalar::Scalar;
    let blinding = Scalar::from_bytes_mod_order(test_blinding());
    let comm = Commitment::new(value, blinding);
    *comm.as_bytes()
}

/// Deploy escrow contract and init. Returns client.
fn setup_contract<'a>(
    env: &'a Env,
    admin: &Address,
    arbitrator: &Address,
    verifier_key: &BytesN<32>,
) -> UmbraEscrowClient<'a> {
    let contract_id = env.register_contract(None, UmbraEscrow);
    let client = UmbraEscrowClient::new(env, &contract_id);
    client.init(admin, arbitrator, verifier_key);
    client
}

// ---------------------------------------------------------------------------
// Test: init
// ---------------------------------------------------------------------------

#[test]
fn test_init_sets_admin_and_arbitrator() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register_contract(None, UmbraEscrow);
    let client = UmbraEscrowClient::new(&env, &contract_id);
    client.init(&admin, &arbitrator, &vk);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_arbitrator(), arbitrator);
    assert_eq!(client.get_verifier_key(), vk);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_init_panics() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register_contract(None, UmbraEscrow);
    let client = UmbraEscrowClient::new(&env, &contract_id);
    client.init(&admin, &arbitrator, &vk);
    client.init(&admin, &arbitrator, &vk);
}

// ---------------------------------------------------------------------------
// Test: create_escrow
// ---------------------------------------------------------------------------

#[test]
fn test_create_escrow_returns_id_and_stores_record() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(125_000_00);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let escrow_id = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    assert_eq!(escrow_id, 0u64);

    let record = client.get_escrow(&escrow_id);
    assert_eq!(record.escrow_id, 0);
    assert_eq!(record.buyer, buyer);
    assert_eq!(record.supplier, supplier);
    assert_eq!(record.commitment, commitment);
    assert_eq!(record.status, EscrowStatus::Active);
}

#[test]
fn test_create_escrow_increments_id() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[3u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(100);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let id1 = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    let id2 = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    assert_eq!(id1, 0);
    assert_eq!(id2, 1);
    assert_eq!(client.next_escrow_id(), 2u64);
}

#[test]
fn test_create_escrow_multisig_condition() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[4u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(250_000);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let signers = Vec::from_array(&env, [signer1.clone(), signer2.clone()]);
    let condition = ReleaseCondition::MultiSig(MultiSigParams {
        required: 2,
        signers,
    });

    let escrow_id = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    assert_eq!(escrow_id, 0u64);
}

// ---------------------------------------------------------------------------
// Test: event contains no private amount (privacy regression)
// ---------------------------------------------------------------------------

#[test]
fn test_escrow_event_contains_no_private_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[5u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    // The private amount that must never appear.
    let private_amount: u64 = 125_000_00;

    let commitment_bytes = make_commitment(private_amount);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let _id = client.create_escrow(&buyer, &supplier, &commitment, &condition);

    // Check all events for the private amount byte pattern.
    let events = env.events().all();
    assert!(!events.is_empty(), "expected at least one event");

    for (_contract_id, _topics, data) in events.iter() {
        let data_xdr = data.clone().to_xdr(&env);
        let data_bytes: Vec<u8> = data_xdr.iter().collect();

        let amount_le = private_amount.to_le_bytes();
        let amount_be = private_amount.to_be_bytes();

        let contains_le = data_bytes.windows(8).any(|w| w == amount_le);
        let contains_be = data_bytes.windows(8).any(|w| w == amount_be);

        assert!(
            !contains_le && !contains_be,
            "private escrow amount found in emitted event — confidentiality regression!"
        );
    }
}

// ---------------------------------------------------------------------------
// Test: get_escrow returns commitment, never amount
// ---------------------------------------------------------------------------

#[test]
fn test_get_escrow_does_not_return_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[6u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(999_999);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let id = client.create_escrow(&buyer, &supplier, &commitment, &condition);

    // The record should contain commitment (opaque), not amount.
    let record = client.get_escrow(&id);
    assert_eq!(record.commitment, commitment);
    // Verify no field in record holds the amount 999_999 in its byte repr.
    let record_xdr = record.clone().to_xdr(&env);
    let record_bytes: Vec<u8> = record_xdr.iter().collect();
    let amount_bytes = 999_999u64.to_le_bytes();
    assert!(
        !record_bytes.windows(8).any(|w| w == amount_bytes),
        "amount leaked in get_escrow return value"
    );
}

// ---------------------------------------------------------------------------
// Test: cancel_escrow
// ---------------------------------------------------------------------------

#[test]
fn test_cancel_escrow_marks_cancelled() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[7u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(1000);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let id = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    client.cancel_escrow(&id);

    let record = client.get_escrow(&id);
    assert_eq!(record.status, EscrowStatus::Cancelled);
}

#[test]
#[should_panic(expected = "escrow not active")]
fn test_cancel_already_cancelled_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[8u8; 32]);
    let buyer = Address::generate(&env);
    let supplier = Address::generate(&env);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);

    let commitment_bytes = make_commitment(500);
    let commitment: BytesN<32> = BytesN::from_array(&env, &commitment_bytes);
    let condition = ReleaseCondition::DeliveryOracle(arbitrator.clone());

    let id = client.create_escrow(&buyer, &supplier, &commitment, &condition);
    client.cancel_escrow(&id);
    client.cancel_escrow(&id); // should panic
}

// ---------------------------------------------------------------------------
// Test: non-existent escrow panics
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "escrow not found")]
fn test_get_nonexistent_escrow_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[9u8; 32]);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);
    client.get_escrow(&999_999);
}

// ---------------------------------------------------------------------------
// Test: version
// ---------------------------------------------------------------------------

#[test]
fn test_version() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let vk: BytesN<32> = BytesN::from_array(&env, &[10u8; 32]);

    let client = setup_contract(&env, &admin, &arbitrator, &vk);
    assert_eq!(client.version(), 1);
}
