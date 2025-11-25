# Simple PowerShell script to check payment_detail table
$dbPath = "$env:LOCALAPPDATA\BinanceP2PManager\p2p_app.db"

Write-Host "Database: $dbPath" -ForegroundColor Cyan
Write-Host ""

if (Test-Path $dbPath) {
    Write-Host "✅ Database exists" -ForegroundColor Green
    
    # Show file size and last modified
    $dbFile = Get-Item $dbPath
    Write-Host "Size: $($dbFile.Length) bytes"
    Write-Host "Last Modified: $($dbFile.LastWriteTime)"
    Write-Host ""
    
    # Use SQLite .NET library via Add-Type (if available)
    # Otherwise just show that DB exists
    Write-Host "📊 To query the database, run this in Tauri app terminal:" -ForegroundColor Yellow
    Write-Host "cargo run --bin check_payments" -ForegroundColor White
    
} else {
    Write-Host "❌ Database not found!" -ForegroundColor Red
}
