#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

pub mod commitments;
pub mod dispute;

#[contract]
pub struct UmbraEscrow;

#[contracttype]
pub enum DataKey {
    Admin,
    Initialized,
}

#[contractimpl]
impl UmbraEscrow {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.events().publish((Symbol::new(&env, "init"),), admin);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn version() -> u32 {
        1
    }
}
