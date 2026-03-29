#![no_std]

//! Timelock governance contract for delayed operational actions.
//!
//! This contract lets the configured admin queue governance actions that can only
//! execute after an action-specific minimum delay. It is intended to slow down
//! sensitive changes such as upgrades, admin transfers, and parameter updates.
//!
//! # Operational runbook
//! Parameter guidance and operational steps for delay selection live in:
//! [`docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook`](../../../docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook)

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

/// Maximum delay allowed (30 days in seconds)
const MAX_DELAY: u64 = 2592000;

/// Different governance action classes supported by the timelock.
///
/// Each action type has a fixed minimum delay returned by [`ActionType::get_delay`].
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionType {
    AdminChange = 0,
    ParameterChange = 1,
    Upgrade = 2,
    FeeChange = 3,
}

impl ActionType {
    /// Get the minimum execution delay for this action type, in seconds.
    pub fn get_delay(&self) -> u64 {
        match self {
            ActionType::AdminChange => 172800,    // 2 days
            ActionType::ParameterChange => 86400, // 1 day
            ActionType::Upgrade => 259200,        // 3 days
            ActionType::FeeChange => 86400,       // 1 day
        }
    }
}

/// Stored metadata for a queued timelock action.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedAction {
    pub id: u64,
    pub action_type: ActionType,
    pub target: Address,
    pub data: String,
    pub queued_at: u64,
    pub executable_at: u64,
    pub executed: bool,
    pub cancelled: bool,
}

/// Contract errors returned by the timelock contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    ActionNotFound = 2,
    ActionAlreadyExecuted = 3,
    ActionCancelled = 4,
    DelayNotMet = 5,
    DelayTooShort = 6,
    DelayTooLong = 7,
    ActionAlreadyCancelled = 8,
    CannotCancelExecutedAction = 9,
    InvalidActionType = 10,
    ArithmeticOverflow = 11,
}

/// Storage keys used by the timelock contract.
#[contracttype]
pub enum StorageKey {
    Admin,
    ActionCounter,
    Action(u64),
    ActionIds,
}

#[contract]
/// Timelock contract that enforces delayed execution for operational changes.
///
/// Security model:
/// - only the stored admin can queue or cancel actions, enforced with `require_auth`
/// - anyone may execute a queued action once its delay has elapsed
/// - delay bounds are constrained by action type minimums and a global maximum
///
/// Runbook reference:
/// [`docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook`](../../../docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook)
pub struct TimelockContract;

#[contractimpl]
impl TimelockContract {
    /// Initialize the contract with an admin.
    ///
    /// This is a single-use setup step that establishes the sole authority allowed
    /// to queue and cancel timelocked actions.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&StorageKey::Admin) {
            panic!("Contract already initialized");
        }

        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::ActionCounter, &0u64);

        let empty_vec: Vec<u64> = Vec::new(&env);
        env.storage()
            .instance()
            .set(&StorageKey::ActionIds, &empty_vec);
    }

    /// Queue a new action with timelock.
    ///
    /// # Arguments
    /// * `action_type` - Type of action being queued.
    /// * `target` - Target address for the action.
    /// * `data` - Action data/parameters as string.
    /// * `delay` - Custom delay in seconds (must be >= action type minimum).
    ///
    /// # Returns
    /// * Result containing the new Action ID.
    ///
    /// # Errors
    /// * `Error::Unauthorized` - Caller is not the admin.
    /// * `Error::DelayTooShort` - `delay` is less than the action type minimum.
    /// * `Error::DelayTooLong` - `delay` exceeds `MAX_DELAY`.
    /// * `Error::ArithmeticOverflow` - Generated ID or timestamp overflows.
    ///
    /// # Security Notes
    /// - **Trust Boundary**: Only the admin can mutate the action queue.
    /// - **Clock Dependency**: Execution time is calculated relative to `env.ledger().timestamp()`.
    ///   Soroban guarantees ledger timestamp monotonicity.
    pub fn queue_action(
        env: Env,
        action_type: ActionType,
        target: Address,
        data: String,
        delay: u64,
    ) -> Result<u64, Error> {
        let admin: Address = env.storage().instance().get(&StorageKey::Admin).unwrap();
        admin.require_auth();

        // Validate delay
        let min_delay = action_type.get_delay();
        if delay < min_delay {
            return Err(Error::DelayTooShort);
        }
        if delay > MAX_DELAY {
            return Err(Error::DelayTooLong);
        }

        // Get and increment counter
        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&StorageKey::ActionCounter)
            .unwrap();
        counter = counter.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&StorageKey::ActionCounter, &counter);

        // Create queued action
        let current_time = env.ledger().timestamp();
        let executable_at = current_time
            .checked_add(delay)
            .ok_or(Error::ArithmeticOverflow)?;

        let action = QueuedAction {
            id: counter,
            action_type,
            target: target.clone(),
            data: data.clone(),
            queued_at: current_time,
            executable_at,
            executed: false,
            cancelled: false,
        };

        // Store action
        env.storage()
            .persistent()
            .set(&StorageKey::Action(counter), &action);

        // Add to action IDs list
        let mut action_ids: Vec<u64> = env
            .storage()
            .instance()
            .get(&StorageKey::ActionIds)
            .unwrap();
        action_ids.push_back(counter);
        env.storage()
            .instance()
            .set(&StorageKey::ActionIds, &action_ids);

        // Emit event
        env.events().publish(
            (symbol_short!("queued"), counter),
            (action_type, target, data, executable_at),
        );

        Ok(counter)
    }

    /// Execute a queued action after the delay has passed.
    ///
    /// Anyone can execute a queued action once the delay has elapsed. This makes
    /// execution liveness independent from the admin being online at the deadline.
    ///
    /// # Arguments
    /// * `action_id` - ID of the action to execute.
    ///
    /// # Errors
    /// * `Error::ActionNotFound` - ID does not match any queued action.
    /// * `Error::ActionAlreadyExecuted` - Action has already been processed.
    /// * `Error::ActionCancelled` - Action was cancelled by the admin.
    /// * `Error::DelayNotMet` - Current ledger timestamp is before `executable_at`.
    ///
    /// # Security Notes
    /// - **Clock Skew**: Execution eligibility is strictly checked against the current ledger 
    ///   timestamp. Because ledger timestamps are validator-driven, execution may be 
    ///   delayed by ledger closing times (approx. 5s).
    pub fn execute_action(env: Env, action_id: u64) -> Result<(), Error> {
        let mut action: QueuedAction = env
            .storage()
            .persistent()
            .get(&StorageKey::Action(action_id))
            .ok_or(Error::ActionNotFound)?;

        // Check if already executed
        if action.executed {
            return Err(Error::ActionAlreadyExecuted);
        }

        // Check if cancelled
        if action.cancelled {
            return Err(Error::ActionCancelled);
        }

        // Check if delay has passed
        let current_time = env.ledger().timestamp();
        if current_time < action.executable_at {
            return Err(Error::DelayNotMet);
        }

        // Mark as executed
        action.executed = true;
        env.storage()
            .persistent()
            .set(&StorageKey::Action(action_id), &action);

        // Emit event
        env.events().publish(
            (symbol_short!("executed"), action_id),
            (action.action_type, action.target.clone(), current_time),
        );

        Ok(())
    }

    /// Cancel a queued action.
    ///
    /// Only the admin can cancel actions, and only before they are executed.
    ///
    /// # Arguments
    /// * `action_id` - ID of the action to cancel.
    ///
    /// # Errors
    /// * `Error::Unauthorized` - Caller is not the admin.
    /// * `Error::ActionNotFound` - ID does not match any queued action.
    /// * `Error::CannotCancelExecutedAction` - Action has already been executed.
    /// * `Error::ActionAlreadyCancelled` - Action was already cancelled.
    ///
    /// # Security Notes
    /// - **Trust Boundary**: Only the admin can cancel actions, ensuring malicious or erroneous 
    ///   queued actions can be neutralized before execution.
    pub fn cancel_action(env: Env, action_id: u64) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&StorageKey::Admin).unwrap();
        admin.require_auth();

        let mut action: QueuedAction = env
            .storage()
            .persistent()
            .get(&StorageKey::Action(action_id))
            .ok_or(Error::ActionNotFound)?;

        // Check if already executed
        if action.executed {
            return Err(Error::CannotCancelExecutedAction);
        }

        // Check if already cancelled
        if action.cancelled {
            return Err(Error::ActionAlreadyCancelled);
        }

        // Mark as cancelled
        action.cancelled = true;
        env.storage()
            .persistent()
            .set(&StorageKey::Action(action_id), &action);

        // Emit event
        env.events().publish(
            (symbol_short!("cancelled"), action_id),
            (action.action_type, action.target.clone()),
        );

        Ok(())
    }

    /// Get details of a queued action.
    ///
    /// # Arguments
    /// * `action_id` - ID of the action
    ///
    /// # Returns
    /// * QueuedAction details
    pub fn get_action(env: Env, action_id: u64) -> Result<QueuedAction, Error> {
        env.storage()
            .persistent()
            .get(&StorageKey::Action(action_id))
            .ok_or(Error::ActionNotFound)
    }

    /// Get all queued action IDs.
    ///
    /// # Returns
    /// * Vector of action IDs
    pub fn get_all_actions(env: Env) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&StorageKey::ActionIds)
            .unwrap_or(Vec::new(&env))
    }

    /// Get pending actions (not executed and not cancelled).
    ///
    /// # Returns
    /// * Vector of pending action IDs
    pub fn get_pending_actions(env: Env) -> Vec<u64> {
        let all_ids: Vec<u64> = Self::get_all_actions(env.clone());
        let mut pending = Vec::new(&env);

        for id in all_ids.iter() {
            if let Some(action) = env
                .storage()
                .persistent()
                .get::<StorageKey, QueuedAction>(&StorageKey::Action(id))
            {
                if !action.executed && !action.cancelled {
                    pending.push_back(id);
                }
            }
        }

        pending
    }

    /// Get executable actions (pending and past delay).
    ///
    /// # Returns
    /// * Vector of executable action IDs
    pub fn get_executable_actions(env: Env) -> Vec<u64> {
        let pending = Self::get_pending_actions(env.clone());
        let mut executable = Vec::new(&env);
        let current_time = env.ledger().timestamp();

        for id in pending.iter() {
            if let Some(action) = env
                .storage()
                .persistent()
                .get::<StorageKey, QueuedAction>(&StorageKey::Action(id))
            {
                if current_time >= action.executable_at {
                    executable.push_back(id);
                }
            }
        }

        executable
    }

    /// Get the current admin address.
    ///
    /// # Returns
    /// * Admin address
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&StorageKey::Admin).unwrap()
    }

    /// Get the minimum delay for an action type.
    ///
    /// # Arguments
    /// * `action_type` - Type of action
    ///
    /// # Returns
    /// * Minimum delay in seconds
    pub fn get_min_delay(env: Env, action_type: ActionType) -> u64 {
        let _ = env;
        action_type.get_delay()
    }

    /// Get the maximum allowed delay.
    ///
    /// # Returns
    /// * Maximum delay in seconds
    pub fn get_max_delay(env: Env) -> u64 {
        let _ = env;
        MAX_DELAY
    }

    /// Get the action counter (total actions queued).
    ///
    /// # Returns
    /// * Total number of actions queued
    pub fn get_action_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&StorageKey::ActionCounter)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
