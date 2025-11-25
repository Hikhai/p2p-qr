# Check payment details in database
$dbPath = "$env:LOCALAPPDATA\BinanceP2PManager\p2p_app.db"

Write-Host "Database path: $dbPath" -ForegroundColor Cyan
Write-Host ""

if (Test-Path $dbPath) {
    Write-Host "✅ Database exists" -ForegroundColor Green
    Write-Host ""
    
    # Check recent payment details
    Write-Host "Recent payment details:" -ForegroundColor Yellow
    sqlite3 $dbPath "SELECT order_number, bank_name, account_no, CASE WHEN qr_code_url IS NOT NULL THEN 'YES' ELSE 'NO' END as has_qr, created_at FROM payment_detail ORDER BY created_at DESC LIMIT 10;"
    
    Write-Host ""
    Write-Host "Total payment details:" -ForegroundColor Yellow
    sqlite3 $dbPath "SELECT COUNT(*) FROM payment_detail;"
    
} else {
    Write-Host "❌ Database not found at: $dbPath" -ForegroundColor Red
}
