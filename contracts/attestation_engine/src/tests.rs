#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_initialize_and_getters() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    let init = e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone())
    });
    assert_eq!(init, Ok(()));

    let stored_admin = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_admin(e.clone()).unwrap()
    });
    let stored_core = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_core_contract(e.clone()).unwrap()
    });

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_core, core);
}

#[test]
fn test_initialize_twice_fails() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    let second = e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone())
    });
    assert_eq!(second, Err(AttestationError::AlreadyInitialized));
}
