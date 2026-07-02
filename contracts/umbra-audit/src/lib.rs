#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

pub mod proof_verifier;

#[contract]
pub struct UmbraAudit;

#[contracttype]
pub enum DataKey {
    Regulator,
    Initialized,
}

#[contractimpl]
impl UmbraAudit {
    pub fn init(env: Env, regulator: Address) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Regulator, &regulator);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.events().publish((Symbol::new(&env, "init"),), regulator);
    }

    pub fn get_regulator(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Regulator)
            .expect("not initialized")
    }

    pub fn version() -> u32 {
        1
    }
}
