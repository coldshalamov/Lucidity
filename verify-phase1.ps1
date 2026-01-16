# Lucidity Phase 1 Verification Script
# Run this to verify the implementation works correctly

Write-Host "=== Lucidity Phase 1 Verification ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build all crates
Write-Host "Step 1: Building all Lucidity crates..." -ForegroundColor Yellow
$buildResult = cargo build -p lucidity-proto -p lucidity-host -p lucidity-client 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Build failed!" -ForegroundColor Red
    Write-Host $buildResult
    exit 1
}
Write-Host "✅ Build successful" -ForegroundColor Green
Write-Host ""

# Step 2: Run unit tests
Write-Host "Step 2: Running unit tests..." -ForegroundColor Yellow
$testResult = cargo test -p lucidity-proto -p lucidity-host 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Tests failed!" -ForegroundColor Red
    Write-Host $testResult
    exit 1
}
Write-Host "✅ All tests passed" -ForegroundColor Green
Write-Host ""

# Step 3: Check for common issues
Write-Host "Step 3: Checking for common issues..." -ForegroundColor Yellow

# Check if port 9797 is already in use
$portInUse = Get-NetTCPConnection -LocalPort 9797 -ErrorAction SilentlyContinue
if ($portInUse) {
    Write-Host "⚠️  Port 9797 is already in use. You may need to:" -ForegroundColor Yellow
    Write-Host "   - Close existing Lucidity/WezTerm instances"
    Write-Host "   - Set LUCIDITY_LISTEN to a different port"
    Write-Host ""
}

Write-Host "✅ Pre-flight checks complete" -ForegroundColor Green
Write-Host ""

# Step 4: Instructions for manual testing
Write-Host "Step 4: Manual Integration Test" -ForegroundColor Yellow
Write-Host ""
Write-Host "To test the full integration:" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. In this terminal, start the GUI:" -ForegroundColor White
Write-Host "   cargo run -p wezterm-gui" -ForegroundColor Gray
Write-Host ""
Write-Host "2. In a NEW terminal, connect the client:" -ForegroundColor White
Write-Host "   cargo run -p lucidity-client -- --addr 127.0.0.1:9797" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Verify:" -ForegroundColor White
Write-Host "   - Client lists available panes" -ForegroundColor Gray
Write-Host "   - Client attaches to a pane" -ForegroundColor Gray
Write-Host "   - Typing in client appears in GUI pane" -ForegroundColor Gray
Write-Host "   - Output from GUI pane appears in client" -ForegroundColor Gray
Write-Host ""

Write-Host "Optional: Test LAN access" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. Set environment variable:" -ForegroundColor White
Write-Host "   `$env:LUCIDITY_LISTEN = '0.0.0.0:9797'" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Start GUI (will listen on all interfaces)" -ForegroundColor White
Write-Host "   cargo run -p wezterm-gui" -ForegroundColor Gray
Write-Host ""
Write-Host "3. From another machine on your LAN:" -ForegroundColor White
Write-Host "   cargo run -p lucidity-client -- --addr <your-ip>:9797" -ForegroundColor Gray
Write-Host ""

Write-Host "=== Verification Complete ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:" -ForegroundColor Green
Write-Host "✅ All crates build successfully"
Write-Host "✅ All unit tests pass"
Write-Host "✅ Ready for manual integration testing"
Write-Host ""
Write-Host "See PHASE1_AUDIT.md for detailed audit results" -ForegroundColor Cyan
Write-Host "See IMPROVEMENTS.md for recommended enhancements" -ForegroundColor Cyan
