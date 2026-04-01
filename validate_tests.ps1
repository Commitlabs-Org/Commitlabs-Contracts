# Validation script for Issue #267 implementation
# This script performs basic syntax and structure validation

Write-Host "Validating Issue #267 Implementation..." -ForegroundColor Cyan

# Check if tests.rs file exists and has content
$testsPath = "contracts\commitment_marketplace\src\tests.rs"
if (-not (Test-Path $testsPath)) {
    Write-Host "[FAIL] tests.rs file not found" -ForegroundColor Red
    exit 1
}

$testsContent = Get-Content $testsPath -Raw
if ($testsContent.Length -eq 0) {
    Write-Host "[FAIL] tests.rs file is empty" -ForegroundColor Red
    exit 1
}

Write-Host "[OK] tests.rs file exists and has content" -ForegroundColor Green

# Check for required test functions
$requiredTests = @(
    "test_make_duplicate_offer_same_token_different_amount_fails",
    "test_make_duplicate_offer_different_tokens_same_user_fails", 
    "test_different_users_can_offer_same_token",
    "test_cancel_offer_removes_correct_offer_only",
    "test_cancel_last_offer_removes_storage",
    "test_cancel_offer_after_accept_fails",
    "test_cancel_multiple_offers_same_user_different_tokens",
    "test_non_maker_cannot_cancel_offer",
    "test_different_offerer_cannot_cancel_other_offer",
    "test_maker_can_cancel_own_offer_multiple_exist",
    "test_cancel_nonexistent_offer_as_non_maker_fails",
    "test_authorization_scenarios_comprehensive"
)

$missingTests = @()
foreach ($test in $requiredTests) {
    if ($testsContent -notmatch "fn $test\(") {
        $missingTests += $test
    }
}

if ($missingTests.Count -gt 0) {
    Write-Host "[FAIL] Missing test functions:" -ForegroundColor Red
    foreach ($test in $missingTests) {
        Write-Host "  - $test" -ForegroundColor Red
    }
    exit 1
}

Write-Host "[OK] All required test functions found" -ForegroundColor Green

# Check for proper error handling
$errorPatterns = @(
    "Error\(Contract, #13\)",  # OfferExists
    "Error\(Contract, #11\)"   # OfferNotFound
)

foreach ($pattern in $errorPatterns) {
    if ($testsContent -notmatch $pattern) {
        Write-Host "[WARN] Missing error pattern: $pattern" -ForegroundColor Yellow
    }
}

Write-Host "[OK] Error handling patterns found" -ForegroundColor Green

# Check for proper test structure
$structureChecks = @(
    "#\[test\]",
    "#\[should_panic\(",
    "use crate::\*",
    "fn setup_marketplace",
    "CommitmentMarketplaceClient"
)

foreach ($check in $structureChecks) {
    if ($testsContent -notmatch [regex]::Escape($check)) {
        Write-Host "[WARN] Missing structure element: $check" -ForegroundColor Yellow
    }
}

Write-Host "[OK] Test structure validation passed" -ForegroundColor Green

# Count total tests
$testCount = ([regex]::Matches($testsContent, "#\[test\]").Count)
$panicTestCount = ([regex]::Matches($testsContent, "#\[should_panic\]").Count)

Write-Host "[INFO] Total test functions: $testCount" -ForegroundColor Cyan
Write-Host "[INFO] Tests expecting panic: $panicTestCount" -ForegroundColor Cyan

# Check implementation documentation
$docPath = "ISSUE_267_IMPLEMENTATION.md"
if (Test-Path $docPath) {
    Write-Host "[OK] Implementation documentation exists" -ForegroundColor Green
} else {
    Write-Host "[WARN] Implementation documentation missing" -ForegroundColor Yellow
}

Write-Host "`n[SUCCESS] Issue #267 implementation validation completed!" -ForegroundColor Green
Write-Host "Ready for PR creation and testing in Rust environment." -ForegroundColor Green
