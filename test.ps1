# PowerShell test script for Windows
Write-Host "🧪 Running all tests..." -ForegroundColor Cyan
cargo test --workspace --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ All tests passed!" -ForegroundColor Green
} else {
    Write-Host "❌ Some tests failed" -ForegroundColor Red
    exit 1
}
