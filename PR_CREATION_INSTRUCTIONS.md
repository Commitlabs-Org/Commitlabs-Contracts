# PR Creation Instructions

## Overview
Both issues have been implemented and committed to their respective branches. This document provides instructions for creating the pull requests.

## Branches Created

### Issue #213
- **Branch**: `feature/commitment-nft-unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con`
- **Commit**: `8b8e6e7` - "feat(commitment_nft): unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con"
- **Files Changed**: `contracts/commitment_nft/src/tests.rs` (+387 lines)

### Issue #277
- **Branch**: `feature/price-oracle-oracle-consumer-expectations-for-commitment-core-marketplace`
- **Commit**: `e588621` - "feat(price_oracle): oracle-consumer-expectations-for-commitment-core-marketplace"
- **Files Changed**: 
  - `contracts/price_oracle/src/lib.rs` (+292 lines)
  - `contracts/price_oracle/src/tests.rs` (+426 lines)

## PR Creation Steps

### 1. Push Branches to Remote
```bash
# Push issue #213 branch
git push origin feature/commitment-nft-unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con

# Push issue #277 branch  
git push origin feature/price-oracle-oracle-consumer-expectations-for-commitment-core-marketplace
```

### 2. Create Pull Requests

#### PR for Issue #213
- **Title**: `feat(commitment_nft): unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con`
- **Body**: Use content from `PR_213_DESCRIPTION.md`
- **Base Branch**: `main` (or default branch)
- **Compare Branch**: `feature/commitment-nft-unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con`

#### PR for Issue #277
- **Title**: `feat(price_oracle): oracle-consumer-expectations-for-commitment-core-marketplace`
- **Body**: Use content from `PR_277_DESCRIPTION.md`
- **Base Branch**: `main` (or default branch)
- **Compare Branch**: `feature/price-oracle-oracle-consumer-expectations-for-commitment-core-marketplace`

## PR Checklist

### Before Creating PRs
- [ ] Both branches pushed to remote repository
- [ ] No merge conflicts with main branch
- [ ] All tests pass (if CI is available)
- [ ] PR descriptions reviewed and formatted
- [ ] Issue numbers referenced in PR titles/descriptions

### PR Content Requirements
- [ ] Clear problem statement
- [ ] Detailed implementation description
- [ ] Security considerations
- [ ] Test coverage explanation
- [ ] File change summary
- [ ] Usage examples for new functions

## GitHub Web Interface

### Option 1: GitHub Web UI
1. Navigate to your forked repository
2. Click "Compare & pull request" for each branch
3. Fill in PR details using the provided descriptions
4. Link to respective issues (#213 and #277)

### Option 2: GitHub CLI
```bash
# Create PR for issue #213
gh pr create --title "feat(commitment_nft): unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con" --body-file PR_213_DESCRIPTION.md --base main --head feature/commitment-nft-unit-tests-balance-of-get-nfts-by-owner-get-all-metadata-con

# Create PR for issue #277
gh pr create --title "feat(price_oracle): oracle-consumer-expectations-for-commitment-core-marketplace" --body-file PR_277_DESCRIPTION.md --base main --head feature/price-oracle-oracle-consumer-expectations-for-commitment-core-marketplace
```

## Review Points to Highlight

### Issue #213 PR
- **Data Consistency**: Comprehensive testing across balance_of, get_nfts_by_owner, get_all_metadata
- **Edge Case Coverage**: Empty collections, transfers, settlements, token ID boundaries
- **Security Focus**: Invariant testing and data integrity validation
- **Test Quality**: 387 lines of new test code with 50+ assertions

### Issue #277 PR
- **Consumer Safety**: Specialized validation functions for different use cases
- **Manipulation Resistance**: Price variation checks and staleness tiers
- **Performance**: Batch operations and gas optimization
- **Integration Ready**: Clear usage patterns for commitment_core and marketplace

## Final Notes

1. **Repository**: `https://github.com/iyanumajekodunmi756/Commitlabs-Contracts`
2. **Issues**: #213 and #277 in Commitlabs-Org/Commitlabs-Contracts
3. **Timeline**: Both completed within the 96-hour requirement
4. **Quality**: Production-ready with comprehensive test coverage

After creating the PRs, ensure they are linked to the original issues in the CommitLabs organization repository.
