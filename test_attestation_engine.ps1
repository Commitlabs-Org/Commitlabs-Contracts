Write-Host "Testing attestation_engine compilation..." -ForegroundColor Cyan

# Test attestation_engine compilation
Write-Host "`nChecking attestation_engine..." -ForegroundColor Yellow
cargo check --package attestation_engine 2>&1 | Out-Null

if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] attestation_engine compiles successfully!" -ForegroundColor Green
} else {
    Write-Host "[FAIL] attestation_engine has errors" -ForegroundColor Red
    cargo check --package attestation_engine
    exit 1
}

# Run tests if compilation succeeds
Write-Host "`nRunning attestation_engine tests..." -ForegroundColor Yellow
cargo test --package attestation_engine 2>&1 | Out-Null

if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] All attestation_engine tests passed!" -ForegroundColor Green
} else {
    Write-Host "[FAIL] Some attestation_engine tests failed" -ForegroundColor Red
    cargo test --package attestation_engine
    exit 1
}

Write-Host "`n[SUCCESS] attestation_engine validation complete!" -ForegroundColor Green
