# Timelock Bypass Attempt Matrix

This document enumerates potential bypass attempts against the `time_lock`
contract and explains why they fail.

## Security Model

- Only the admin may queue or cancel actions.
- Anyone may execute actions **only after the delay has passed**.
- Execution state is immutable once executed or cancelled.

---

# Bypass Attempt Matrix

| Attempt | Attack Description | Expected Result | Protection |
|-------|-------------------|---------------|------------|
| Execute before delay | Attacker attempts execution before `executable_at` | Rejected | `DelayNotMet` check |
| Double execution | Attacker executes same action twice | Rejected | `executed` flag |
| Execute cancelled action | Execute after admin cancels | Rejected | `cancelled` flag |
| Cancel executed action | Admin cancels after execution | Rejected | `CannotCancelExecutedAction` |
| Non-admin queue | Unauthorized actor queues governance action | Rejected | `require_auth()` |
| Non-admin cancel | Unauthorized actor cancels action | Rejected | `require_auth()` |
| Overflow delay | Delay > MAX_DELAY | Rejected | `DelayTooLong` |
| Underflow delay | Delay < min delay | Rejected | `DelayTooShort` |
| Timestamp manipulation | Attempt execution immediately after queue | Rejected | timestamp comparison |
| Counter overflow | Action counter overflow | Rejected | `checked_add` |

---

# Trust Boundaries

### Admin
Allowed:
- queue actions
- cancel actions

Not allowed:
- bypass delays
- execute before time

### Public Users
Allowed:
- execute ready actions

Not allowed:
- queue actions
- cancel actions
- mutate state