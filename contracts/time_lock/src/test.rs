#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};

fn create_test_env() -> (Env, Address, Address) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let target = Address::generate(&env);
    (env, admin, target)
}

#[test]
fn test_initialization() {
    let (env, admin, _) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_action_count(), 0);
}

#[test]
#[should_panic]
fn test_double_initialization() {
    let (env, admin, _) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

#[test]
fn test_queue_action_success() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64; // 1 day

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    assert_eq!(action_id, 1);
    assert_eq!(client.get_action_count(), 1);

    let action = client.get_action(&action_id);
    assert_eq!(action.id, 1);
    assert_eq!(action.action_type, ActionType::ParameterChange);
    assert_eq!(action.target, target);
    assert_eq!(action.data, data);
    assert_eq!(action.executed, false);
    assert_eq!(action.cancelled, false);
}

#[test]
fn test_queue_multiple_actions() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data1 = String::from_str(&env, "action1");
    let data2 = String::from_str(&env, "action2");
    let data3 = String::from_str(&env, "action3");

    let id1 = client.queue_action(&ActionType::ParameterChange, &target, &data1, &86400);
    let id2 = client.queue_action(&ActionType::FeeChange, &target, &data2, &86400);
    let id3 = client.queue_action(&ActionType::Upgrade, &target, &data3, &259200);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
    assert_eq!(client.get_action_count(), 3);

    let all_actions = client.get_all_actions();
    assert_eq!(all_actions.len(), 3);
}

#[test]
fn test_delay_validation_too_short() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    
    // Try to queue with delay shorter than minimum for AdminChange (2 days)
    let result = client.try_queue_action(
        &ActionType::AdminChange,
        &target,
        &data,
        &86400, // 1 day - too short
    );

    assert_eq!(result, Err(Ok(Error::DelayTooShort)));
}

#[test]
fn test_delay_validation_too_long() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    
    // Try to queue with delay longer than maximum (30 days)
    let result = client.try_queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &2592001, // 30 days + 1 second - too long
    );

    assert_eq!(result, Err(Ok(Error::DelayTooLong)));
}

#[test]
fn test_execute_action_success() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    // Fast forward time past the delay
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + delay + 1;
    });

    // Anyone can execute after delay
    client.execute_action(&action_id);

    let action = client.get_action(&action_id);
    assert_eq!(action.executed, true);
}

#[test]
fn test_execute_action_before_delay() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    // Try to execute before delay
    let result = client.try_execute_action(&action_id);
    assert_eq!(result, Err(Ok(Error::DelayNotMet)));
}

#[test]
fn test_execute_already_executed_action() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    // Fast forward and execute
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + delay + 1;
    });

    client.execute_action(&action_id);

    // Try to execute again
    let result = client.try_execute_action(&action_id);
    assert_eq!(result, Err(Ok(Error::ActionAlreadyExecuted)));
}

#[test]
fn test_cancel_action_success() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    client.cancel_action(&action_id);

    let action = client.get_action(&action_id);
    assert_eq!(action.cancelled, true);
}

#[test]
fn test_cancel_already_cancelled_action() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    client.cancel_action(&action_id);

    // Try to cancel again
    let result = client.try_cancel_action(&action_id);
    assert_eq!(result, Err(Ok(Error::ActionAlreadyCancelled)));
}

#[test]
fn test_cancel_executed_action() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    // Fast forward and execute
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + delay + 1;
    });

    client.execute_action(&action_id);

    // Try to cancel
    let result = client.try_cancel_action(&action_id);
    assert_eq!(result, Err(Ok(Error::CannotCancelExecutedAction)));
}

#[test]
fn test_execute_cancelled_action() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    client.cancel_action(&action_id);

    // Fast forward time
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + delay + 1;
    });

    // Try to execute cancelled action
    let result = client.try_execute_action(&action_id);
    assert_eq!(result, Err(Ok(Error::ActionCancelled)));
}

#[test]
fn test_get_pending_actions() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");

    // Queue 3 actions
    let id1 = client.queue_action(&ActionType::ParameterChange, &target, &data, &86400);
    let id2 = client.queue_action(&ActionType::FeeChange, &target, &data, &86400);
    let id3 = client.queue_action(&ActionType::Upgrade, &target, &data, &259200);

    let pending = client.get_pending_actions();
    assert_eq!(pending.len(), 3);

    // Cancel one
    client.cancel_action(&id2);

    let pending = client.get_pending_actions();
    assert_eq!(pending.len(), 2);
    assert!(pending.contains(&id1));
    assert!(pending.contains(&id3));

    // Execute one
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + 86400 + 1;
    });
    client.execute_action(&id1);

    let pending = client.get_pending_actions();
    assert_eq!(pending.len(), 1);
    assert!(pending.contains(&id3));
}

#[test]
fn test_get_executable_actions() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");

    // Queue actions with different delays
    let id1 = client.queue_action(&ActionType::ParameterChange, &target, &data, &86400); // 1 day
    let id2 = client.queue_action(&ActionType::AdminChange, &target, &data, &172800); // 2 days
    let id3 = client.queue_action(&ActionType::Upgrade, &target, &data, &259200); // 3 days

    // Initially no actions are executable
    let executable = client.get_executable_actions();
    assert_eq!(executable.len(), 0);

    // Fast forward 1 day + 1 second
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + 86400 + 1;
    });

    let executable = client.get_executable_actions();
    assert_eq!(executable.len(), 1);
    assert!(executable.contains(&id1));

    // Fast forward to 2 days + 1 second total
    env.ledger().with_mut(|li| {
        li.timestamp = 172800 + 2;
    });

    let executable = client.get_executable_actions();
    assert_eq!(executable.len(), 2);
    assert!(executable.contains(&id1));
    assert!(executable.contains(&id2));

    // Fast forward to 3 days + 1 second total
    env.ledger().with_mut(|li| {
        li.timestamp = 259200 + 2;
    });

    let executable = client.get_executable_actions();
    assert_eq!(executable.len(), 3);
    assert!(executable.contains(&id1));
    assert!(executable.contains(&id2));
    assert!(executable.contains(&id3));
}

#[test]
fn test_different_action_type_delays() {
    let (env, admin, _target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    // Test minimum delays for each action type
    assert_eq!(client.get_min_delay(&ActionType::ParameterChange), 86400); // 1 day
    assert_eq!(client.get_min_delay(&ActionType::FeeChange), 86400); // 1 day
    assert_eq!(client.get_min_delay(&ActionType::AdminChange), 172800); // 2 days
    assert_eq!(client.get_min_delay(&ActionType::Upgrade), 259200); // 3 days
}

#[test]
fn test_action_not_found() {
    let (env, admin, _) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    let result = client.try_get_action(&999);
    assert_eq!(result, Err(Ok(Error::ActionNotFound)));
}

#[test]
fn test_complex_workflow() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");

    // Queue multiple actions of different types
    let param_id = client.queue_action(&ActionType::ParameterChange, &target, &data, &86400);
    let fee_id = client.queue_action(&ActionType::FeeChange, &target, &data, &86400);
    let admin_id = client.queue_action(&ActionType::AdminChange, &target, &data, &172800);
    let upgrade_id = client.queue_action(&ActionType::Upgrade, &target, &data, &259200);

    // Verify all are pending
    let pending = client.get_pending_actions();
    assert_eq!(pending.len(), 4);

    // Cancel the fee change
    client.cancel_action(&fee_id);
    assert_eq!(client.get_pending_actions().len(), 3);

    // Fast forward 1 day
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + 86400 + 1;
    });

    // Execute parameter change
    client.execute_action(&param_id);
    assert_eq!(client.get_pending_actions().len(), 2);
    
    // Verify executable actions
    let executable = client.get_executable_actions();
    assert_eq!(executable.len(), 0); // Only admin and upgrade remain, both need more time

    // Fast forward to 2 days
    env.ledger().with_mut(|li| {
        li.timestamp = 172800 + 2;
    });

    // Execute admin change
    client.execute_action(&admin_id);
    assert_eq!(client.get_pending_actions().len(), 1);

    // Fast forward to 3 days
    env.ledger().with_mut(|li| {
        li.timestamp = 259200 + 2;
    });

    // Execute upgrade
    client.execute_action(&upgrade_id);
    assert_eq!(client.get_pending_actions().len(), 0);

    // Verify final state
    let all_actions = client.get_all_actions();
    assert_eq!(all_actions.len(), 4);

    assert!(client.get_action(&param_id).executed);
    assert!(client.get_action(&fee_id).cancelled);
    assert!(client.get_action(&admin_id).executed);
    assert!(client.get_action(&upgrade_id).executed);
}

#[test]
fn test_edge_case_exact_delay_time() {
    let (env, admin, target) = create_test_env();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    let data = String::from_str(&env, "test_data");
    let delay = 86400u64;

    let action_id = client.queue_action(
        &ActionType::ParameterChange,
        &target,
        &data,
        &delay,
    );

    // Fast forward to exactly the delay time (not past it)
    env.ledger().with_mut(|li| {
        li.timestamp = li.timestamp + delay;
    });

    // Should be executable at exactly the delay time
    client.execute_action(&action_id);
    assert!(client.get_action(&action_id).executed);
}

#[test]
fn test_max_delay_constant() {
    let env = Env::default();
    let contract_id = env.register_contract(None, TimelockContract);
    let client = TimelockContractClient::new(&env, &contract_id);

    assert_eq!(client.get_max_delay(), 2592000); // 30 days
}