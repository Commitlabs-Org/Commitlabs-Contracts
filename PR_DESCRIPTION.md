# Description

Closes #96

## Changes proposed

### What were you told to do?

The task was to implement persistent storage for the Commitment NFT contract with the following requirements:

- Implement storage structures for Admin address (singleton), Token counter, NFT data (token_id -> CommitmentNFT), Owner mapping, Metadata storage, Active status, and Total supply tracking.
- Use Soroban's Storage API with appropriate key types and key derivation for efficiency.
- Handle storage optimization and cost considerations.
- Implement the `initialize()` function to set the admin and initialize storage.
- Create storage helpers for reading/writing NFT data and ownership management.
- Ensure data integrity and efficient lookups.

### What did I do?

**Implemented Storage Architecture:**

- Defined a `DataKey` enum to manage all contract state.
- Utilized both `instance` storage (for shared/global state like Admin and counters) and `persistent` storage (for individual NFT data) to optimize for gas and durability.

**Modularized Storage Logic:**

- Created a dedicated `storage` module to abstract the Soroban Storage API.
- Implemented helpers for incrementing counters, managing ownership mappings, and retrieving metadata.
- Ensured proper data sync between the primary `CommitmentNFT` struct and secondary lookups (like `Owner`).

**Developed Contract Functionality:**

- **initialize**: Implemented a secure initialization routine that sets the admin and resets counters, protected against multiple calls.
- **mint**: Added logic to generate unique token IDs, calculate ledger-based expiration timestamps, and store comprehensive NFT records.
- **transfer**: Implemented ownership transfers with `require_auth` checks and dual-state updates (both the mapping and the NFT record).
- **settle**: Added logic to mark commitments as inactive once the expiration timestamp has passed.

**Added Utility Functions:**

- Implemented `total_supply()`, `get_admin()`, and `current_token_id()` as public visibility methods for easier querying of contract state.

**Verification and Quality:**

- Wrote initial unit tests for initialization and minting to confirm data is correctly persisted and retrieved.
- Cleaned up unused imports and variables to ensure a clean build.
- Verified successful builds and test runs using the Soroban SDK.

## Check List

🚨Please review the contribution guideline for this repository.

- [x] My code follows the code style of this project.
- [x] This PR does not contain plagiarized content.
- [x] The title and description of the PR is clear and explains the approach.
- [x] I am making a pull request against the dev branch (left side).
- [x] My commit messages styles matches our requested structure.
- [x] My code additions will fail neither code linting checks nor unit test.
- [x] I am only making changes to files I was requested to.

## Screenshots/Videos

_(N/A - Smart Contract Implementation)_
