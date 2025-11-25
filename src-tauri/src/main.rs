#![cfg_attr(all(not(debug_assertions), target_os="windows"), windows_subsystem="windows")]

mod crypto;
mod db;
mod api { pub mod c2c_api_client; pub mod credentials; pub mod sync_engine; }
mod orders { pub mod repo; pub mod stage_map; }

use std::sync::Arc;
use std::fs;
use tokio::sync::RwLock;
use tauri::{State, Manager};
use tauri::Emitter;
use anyhow::Result;

// HTTP server imports
use axum::{
    extract::{Json, State as AxumState},
    http::{StatusCode, HeaderMap},
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use serde::{Deserialize, Serialize};

use crypto::CryptoCtx;
use db::Db;
use api::credentials::CredentialsRepo;
use api::c2c_api_client::C2CApiClient;
use api::sync_engine::SyncEngine;
use orders::repo::OrderRepo;

// App context for Phase 4 (WS extension capture will be reintegrated with DB in Phase 6)
struct AppCtx {
    order_repo: Arc<OrderRepo>,
    creds_repo: Arc<CredentialsRepo>,
    api_client: Arc<RwLock<Option<C2CApiClient>>>,
}

// State for Axum HTTP server (extension -> app bridge)
#[derive(Clone)]
struct HttpAppState {
    order_repo: Arc<OrderRepo>,
    handle: tauri::AppHandle,
}

// HTTP API structs
#[derive(Deserialize, Serialize)]
struct ExtensionRequest {
    #[serde(rename = "type")]
    request_type: String,
    data: serde_json::Value,
    timestamp: i64,
    source: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
    timestamp: i64,
}

#[tauri::command]
async fn store_api_credentials(state: State<'_, AppCtx>, label: String, api_key: String, api_secret: String) -> Result<(), String> {
    state.creds_repo.store(&label, &api_key, &api_secret).await.map_err(|e| e.to_string())?;
    {
        let mut guard = state.api_client.write().await;
        let client = C2CApiClient::new(api_key, api_secret);
        
        // Try to sync time, but don't fail if it doesn't work (might be network issue)
        println!("[STORE_CREDS] Syncing time with Binance...");
        if let Err(e) = client.sync_time().await {
            println!("[STORE_CREDS] Warning: Failed to sync time: {}. Will retry on first API call.", e);
        }
        
        *guard = Some(client);
    }
    Ok(())
}

#[tauri::command]
async fn check_api_credentials(state: State<'_, AppCtx>) -> Result<bool, String> {
    let guard = state.api_client.read().await;
    Ok(guard.is_some())
}

#[tauri::command]
async fn get_saved_credentials(state: State<'_, AppCtx>) -> Result<Option<(String, String)>, String> {
    state.creds_repo.latest().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_api_credentials(state: State<'_, AppCtx>) -> Result<String, String> {
    let client = {
        let guard = state.api_client.read().await;
        if guard.is_none() { return Err("Chưa có API credentials".into()); }
        guard.as_ref().unwrap().clone()
    };
    
    // Sync time before testing - with retry
    println!("[TEST_CREDS] Syncing time with Binance...");
    for attempt in 1..=3 {
        match client.sync_time().await {
            Ok(_) => {
                println!("[TEST_CREDS] Time sync successful on attempt {}", attempt);
                break;
            }
            Err(e) if attempt < 3 => {
                println!("[TEST_CREDS] Time sync failed on attempt {}: {}. Retrying...", attempt, e);
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(e) => {
                return Err(format!("Failed to sync time after 3 attempts: {}", e));
            }
        }
    }
    
    // Small delay to ensure time is synced
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let now = chrono::Utc::now().timestamp_millis();
    let start = now - 5 * 60 * 1000;
    let res = client.list_user_order_history("BUY", start, now, 1, 1).await.map_err(|e| e.to_string())?;
    Ok(res.to_string())
}

#[tauri::command]
async fn force_initial_sync(state: State<'_, AppCtx>, days: i64) -> Result<String, String> {
    let (client, repo) = {
        let guard = state.api_client.read().await;
        if guard.is_none() { return Err("Chưa cấu hình API client".into()); }
        (guard.as_ref().unwrap().clone(), state.order_repo.clone())
    };
    
    // Sync time before syncing orders
    println!("[FORCE_SYNC] Syncing time with Binance...");
    client.sync_time().await.map_err(|e| format!("Failed to sync time: {}", e))?;
    
    let engine = SyncEngine::new(&client, &repo);
    engine.force_initial_sync(days).await.map_err(|e| e.to_string())?;
    Ok("SYNC_OK".into())
}

#[tauri::command]
async fn list_orders_from_db(state: State<'_, AppCtx>, limit: i64) -> Result<Vec<orders::repo::OrderRow>, String> {
    state.order_repo.list_orders(limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn force_sync_recent(state: State<'_, AppCtx>) -> Result<String, String> {
    let (client, repo) = {
        let guard = state.api_client.read().await;
        if guard.is_none() { return Err("Chưa cấu hình API client".into()); }
        (guard.as_ref().unwrap().clone(), state.order_repo.clone())
    };
    
    // Sync time periodically
    let _ = client.sync_time().await; // Don't fail if sync fails, just log
    
    let engine = SyncEngine::new(&client, &repo);
    
    // First do active poll to update in-progress orders
    engine.active_poll().await.map_err(|e| e.to_string())?;
    
    // Then do incremental sync to get any new orders
    engine.incremental_sync().await.map_err(|e| e.to_string())?;
    
    Ok("SYNC_RECENT_OK".into())
}

#[tauri::command]
async fn cleanup_old_payment_details(state: State<'_, AppCtx>) -> Result<String, String> {
    let pool = state.order_repo.pool();
    
    // Remove payment details for orders that are no longer in "processing" status
    let result = sqlx::query(r#"
        DELETE FROM order_payment_detail 
        WHERE order_number NOT IN (
            SELECT order_number FROM orders 
            WHERE status_code IN (1, 2, 3)  -- Only keep for processing orders
        )
        OR purge_after < ?
    "#)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(format!("Cleaned up {} old payment records", result.rows_affected()))
}

#[tauri::command]
async fn save_payment_detail_from_extension(
    state: State<'_, AppCtx>, 
    order_number: String,
    account_name: Option<String>,
    account_no: Option<String>, 
    bank_name: Option<String>,
    sub_bank: Option<String>,
    qr_code_url: Option<String>,
    amount: Option<String>,
    transfer_content: Option<String>
) -> Result<String, String> {
    let pool = state.order_repo.pool();
    
    println!("[DEBUG] Saving payment detail for order: {}", order_number);
    println!("  - amount: {:?}", amount);
    println!("  - transfer_content: {:?}", transfer_content);
    
    sqlx::query(r#"
        INSERT OR REPLACE INTO order_payment_detail 
        (order_number, account_name, account_no, bank_name, sub_bank, qr_code_url, amount, transfer_content, captured_at, purge_after)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#)
    .bind(&order_number)
    .bind(account_name.clone())
    .bind(account_no.clone())
    .bind(bank_name.clone())
    .bind(sub_bank.clone())
    .bind(qr_code_url.clone())
    .bind(amount.clone())
    .bind(transfer_content.clone())
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(chrono::Utc::now().timestamp_millis() + 24*60*60*1000)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(format!("Payment detail saved for order {}", order_number))
}

#[tauri::command]
async fn clear_all_data(_app_handle: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;
    
    println!("[CLEAR_DATA] Starting shutdown → delete sequence (NO auto-restart)...");
    
    // Database is in AppData directory (same as main function)
    let app_data_dir = dirs::data_local_dir()
        .ok_or("Failed to get AppData directory")?
        .join("BinanceP2PManager");
    
    // Database files in AppData
    let db_path = app_data_dir.join("p2p_app.db");
    let db_wal = app_data_dir.join("p2p_app.db-wal");
    let db_shm = app_data_dir.join("p2p_app.db-shm");
    
    println!("[CLEAR_DATA] DB path: {:?}", db_path);
    
    // Convert paths to strings with proper escaping
    let db_str = db_path.to_string_lossy().replace("/", "\\");
    let wal_str = db_wal.to_string_lossy().replace("/", "\\");
    let shm_str = db_shm.to_string_lossy().replace("/", "\\");
    
    // Spawn a background process that will:
    // 1. Wait for app to close
    // 2. Delete database files  
    // User will manually restart the app
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            r#"Start-Sleep -Seconds 2; Remove-Item -Path '{}','{}','{}' -ErrorAction SilentlyContinue"#,
            db_str, wal_str, shm_str
        );
        
        println!("[CLEAR_DATA] PowerShell script: {}", script);
        
        Command::new("powershell")
            .args(&["-WindowStyle", "Hidden", "-Command", &script])
            .spawn()
            .map_err(|e| format!("Failed to spawn cleanup: {}", e))?;
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let script = format!(
            r#"sleep 2 && rm -f '{}' '{}' '{}'"#,
            db_str, wal_str, shm_str
        );
        
        Command::new("sh")
            .args(&["-c", &script])
            .spawn()
            .map_err(|e| format!("Failed to spawn cleanup: {}", e))?;
    }
    
    println!("[CLEAR_DATA] Cleanup process spawned. App will exit now.");
    println!("[CLEAR_DATA] Please manually restart the app after 2 seconds.");
    
    // Give user a moment to see the message
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Exit the app - user will manually restart
    std::process::exit(0);
}





// HTTP endpoint for extension communication
// Map Vietnamese bank names to BIN codes for VietQR
// CRITICAL ORDER: Check banks with potential substring conflicts FIRST
fn get_bank_bin(bank_name: &str) -> Option<&'static str> {
    let name_lower = bank_name.to_lowercase();
    
    // ====== PRIORITY 1: Banks with "combank" substring - CHECK FIRST ======
    // Must check BEFORE MB Bank to avoid false matches (Techcombank/Sacombank contain "mb")
    // Techcombank - Ngân hàng TMCP Kỹ Thương Việt Nam
    if name_lower.contains("techcombank") || name_lower.contains("tech combank") || name_lower.contains("kỹ thương") || name_lower.contains("ky thuong") || name_lower == "tcb" { 
        return Some("970407"); 
    }
    // Sacombank - Ngân hàng TMCP Sài Gòn Thương Tín
    if name_lower.contains("sacombank") || name_lower.contains("sacom bank") || name_lower.contains("sài gòn thương tín") || name_lower.contains("sai gon thuong tin") || name_lower == "stb" { 
        return Some("970403"); 
    }
    
    // ====== PRIORITY 2: Vietcombank - CHECK BEFORE MB Bank ======
    if name_lower.contains("vietcombank") || name_lower.contains("ngoại thương") || name_lower.contains("ngoai thuong") || name_lower == "vcb" { 
        return Some("970436"); 
    }
    
    // ====== PRIORITY 3: MB Bank - Ngân hàng Quân Đội ======
    // Check AFTER Techcombank/Sacombank/Vietcombank
    // Match: MB, mb, MBBank, MB Bank, MBank, Quân Đội
    if name_lower == "mb" ||  // Exact match first
       name_lower == "mbank" ||
       name_lower.contains("mbbank") || 
       name_lower.contains("mb bank") || 
       name_lower.contains("ngân hàng mb") || 
       name_lower.contains("quân đội") ||
       name_lower.contains("ngan hang mb") ||
       (name_lower.starts_with("mb ") && !name_lower.contains("combank")) ||  // "MB <something>" but not "MB combank"
       (name_lower.ends_with(" mb") && !name_lower.contains("combank")) {     // "<something> MB" but not "<something>combank"
        return Some("970422");
    }
    
    // ====== PRIORITY 4: Other banks with short codes - exact match first ======
    if name_lower == "acb" { return Some("970416"); }
    if name_lower == "bidv" || name_lower == "bid" { return Some("970418"); }
    if name_lower == "msb" { return Some("970426"); }
    if name_lower == "ncb" { return Some("970419"); }
    if name_lower == "ocb" { return Some("970448"); }
    if name_lower == "scb" { return Some("970429"); }
    if name_lower == "shb" { return Some("970443"); }
    if name_lower == "ivb" { return Some("970434"); }
    if name_lower == "vpb" { return Some("970432"); }
    if name_lower == "hd" { return Some("970437"); }
    
    // ====== PRIORITY 5: Regular banks (alphabetically sorted) ======
    // ABBank - Ngân hàng TMCP An Bình
    if name_lower.contains("abbank") || name_lower.contains("ab bank") || name_lower.contains("an bình") || name_lower.contains("an binh") { return Some("970425"); }
    
    // ACB - Ngân hàng TMCP Á Châu
    if name_lower.contains("acb") || name_lower.contains("á châu") || name_lower.contains("a chau") || name_lower.contains("asia commercial") { return Some("970416"); }
    
    // Agribank - Ngân hàng Nông nghiệp và Phát triển Nông thôn Việt Nam
    if name_lower.contains("agribank") || name_lower.contains("agri") || name_lower.contains("nông nghiệp") || name_lower.contains("nong nghiep") || name_lower.contains("vbard") { return Some("970405"); }
    
    // BacABank - Ngân hàng TMCP Bắc Á
    if name_lower.contains("bacabank") || name_lower.contains("bac a") || name_lower.contains("bắc á") || name_lower.contains("bac a") { return Some("970409"); }
    
    // BaoVietBank - Ngân hàng TMCP Bảo Việt
    if name_lower.contains("baoviet") || name_lower.contains("bảo việt") || name_lower.contains("bao viet") || name_lower.contains("bvb") { return Some("970438"); }
    
    // BIDV - Ngân hàng TMCP Đầu tư và Phát triển Việt Nam
    if name_lower.contains("bidv") || name_lower.contains("đầu tư") || name_lower.contains("dau tu") || name_lower.contains("bid") { return Some("970418"); }
    
    // BVBank - Ngân hàng TMCP Bản Việt (formerly PGBank)
    if name_lower.contains("bvbank") || name_lower.contains("bản việt") || name_lower.contains("ban viet") { return Some("970433"); }
    
    // Cake - Ngân hàng số CAKE by VPBank
    if name_lower.contains("cake") { return Some("546034"); }
    
    // CIMB - Ngân hàng TNHH MTV CIMB Việt Nam
    if name_lower.contains("cimb") { return Some("422589"); }
    
    // Co-opBank - Ngân hàng Hợp tác xã Việt Nam
    if name_lower.contains("co-opbank") || name_lower.contains("coopbank") || name_lower.contains("cooperative") || name_lower.contains("hợp tác xã") || name_lower.contains("hop tac xa") { return Some("970446"); }
    
    // Eximbank - Ngân hàng TMCP Xuất Nhập khẩu Việt Nam
    if name_lower.contains("eximbank") || name_lower.contains("exim") || name_lower.contains("xuất nhập khẩu") || name_lower.contains("xuat nhap khau") || name_lower.contains("eib") { return Some("970431"); }
    
    // GPBank - Ngân hàng TMCP Dầu khí Toàn Cầu
    if name_lower.contains("gpbank") || name_lower.contains("gp bank") || name_lower.contains("dầu khí") || name_lower.contains("dau khi") { return Some("970408"); }
    
    // HDBank - Ngân hàng TMCP Phát triển Thành phố Hồ Chí Minh
    if name_lower.contains("hdbank") || name_lower.contains("hd bank") || name_lower.contains("phát triển tp") || name_lower.contains("phat trien tp") || name_lower.contains("phát triển thành phố") || name_lower.contains("phat trien thanh pho") || name_lower.contains("hồ chí minh") || name_lower.contains("ho chi minh") || name_lower.contains("tp hcm") || name_lower.contains("tphcm") { return Some("970437"); }
    
    // HSBC - Ngân hàng TNHH MTV HSBC Việt Nam
    if name_lower.contains("hongkong") || name_lower.contains("hsbc") || name_lower.contains("hong kong") { return Some("458761"); }
    
    // IndovinaBank - Ngân hàng TNHH Indovina
    if name_lower.contains("ivb") || name_lower.contains("indovina") { return Some("970434"); }
    
    // KienLongBank - Ngân hàng TMCP Kiên Long
    if name_lower.contains("kienlongbank") || name_lower.contains("kien long") || name_lower.contains("kiên long") || name_lower.contains("klb") { return Some("970452"); }
    
    // LioBank - Ngân hàng số Lio
    if name_lower.contains("liobank") || name_lower.contains("lio bank") || name_lower == "lio" { return Some("963369"); }
    
    // LienVietPostBank - Ngân hàng TMCP Bưu Điện Liên Việt
    if name_lower.contains("lienviet") || name_lower.contains("lien viet") || name_lower.contains("liên việt") || name_lower.contains("lvbank") || name_lower.contains("lvpb") || name_lower.contains("bưu điện") || name_lower.contains("buu dien") || name_lower.contains("lpbank") { return Some("970449"); }
    
    // MSB - Ngân hàng TMCP Hàng Hải Việt Nam
    if name_lower.contains("msb") || name_lower.contains("hàng hải") || name_lower.contains("hang hai") || name_lower.contains("maritime") { return Some("970426"); }
    
    // NamABank - Ngân hàng TMCP Nam Á
    if name_lower.contains("namabank") || name_lower.contains("nam a") || name_lower.contains("nam á") { return Some("970428"); }
    
    // NCB - Ngân hàng TMCP Quốc Dân
    if name_lower.contains("ncb") || name_lower.contains("quốc dân") || name_lower.contains("quoc dan") || name_lower.contains("national citizen") { return Some("970419"); }
    
    // OCB - Ngân hàng TMCP Phương Đông
    if name_lower.contains("ocb") || name_lower.contains("phương đông") || name_lower.contains("phuong dong") || name_lower.contains("orient") { return Some("970448"); }
    
    // OceanBank - Ngân hàng TMCP Đại Dương
    if name_lower.contains("oceanbank") || name_lower.contains("ocean bank") || name_lower.contains("đại dương") || name_lower.contains("dai duong") { return Some("970414"); }
    
    // PGBank - Ngân hàng TMCP Xăng dầu Petrolimex (now BVBank)
    if name_lower.contains("pgbank") || name_lower.contains("pg bank") || name_lower.contains("xăng dầu") || name_lower.contains("xang dau") || name_lower.contains("petrolimex") { return Some("970430"); }
    
    // PublicBank - Ngân hàng TNHH MTV Public Việt Nam
    if name_lower.contains("publicbank") || name_lower.contains("public bank") || name_lower.contains("pbvn") { return Some("970439"); }
    
    // PVcomBank - Ngân hàng TMCP Đại Chúng Việt Nam
    if name_lower.contains("pvcombank") || name_lower.contains("pvcom") || name_lower.contains("đại chúng") || name_lower.contains("dai chung") { return Some("970412"); }
    
    // SaigonBank - Ngân hàng TMCP Sài Gòn Công Thương
    if name_lower.contains("saigonbank") || name_lower.contains("saigon bank") || name_lower.contains("sài gòn công thương") || name_lower.contains("sai gon cong thuong") || name_lower.contains("sgb") { return Some("970400"); }
    
    // SCB - Ngân hàng TMCP Sài Gòn (already handled above but kept for reference)
    if name_lower.contains("scb") || name_lower.contains("sài gòn") || name_lower.contains("sai gon") { return Some("970429"); }
    
    // SeABank - Ngân hàng TMCP Đông Nam Á
    if name_lower.contains("seabank") || name_lower.contains("sea bank") || name_lower.contains("đông nam á") || name_lower.contains("dong nam a") { return Some("970440"); }
    
    // SHB - Ngân hàng TMCP Sài Gòn - Hà Nội
    if name_lower.contains("shb") || name_lower.contains("sài gòn hà nội") || name_lower.contains("sai gon ha noi") { return Some("970443"); }
    
    // Shinhan - Ngân hàng TNHH MTV Shinhan Việt Nam
    if name_lower.contains("shinhan") || name_lower.contains("shbvn") { return Some("970424"); }
    
    // Standard Chartered - Ngân hàng TNHH MTV Standard Chartered Việt Nam
    if name_lower.contains("standard chartered") || name_lower.contains("scbvl") { return Some("970410"); }
    
    // TPBank - Ngân hàng TMCP Tiên Phong
    if name_lower.contains("tpbank") || 
       name_lower.contains("tp bank") || 
       name_lower.contains("tiên phong") ||
       name_lower.contains("tien phong") ||
       name_lower.contains("tienphong") { 
        return Some("970423"); 
    }
    
    // UOB - Ngân hàng United Overseas Bank Việt Nam
    if name_lower.contains("uob") || name_lower.contains("united overseas") { return Some("970458"); }
    
    // VBSP - Ngân hàng Chính sách Xã hội Việt Nam
    if name_lower.contains("vbsp") || name_lower.contains("chính sách") || name_lower.contains("chinh sach") || name_lower.contains("xã hội") || name_lower.contains("xa hoi") { return Some("999888"); }
    
    // VDB - Ngân hàng Phát triển Việt Nam
    if name_lower.contains("vdb") || name_lower.contains("phát triển việt nam") || name_lower.contains("phat trien viet nam") { return Some("970406"); }
    
    // VIB - Ngân hàng TMCP Quốc tế Việt Nam
    if name_lower.contains("vib") || name_lower.contains("quốc tế") || name_lower.contains("quoc te") || name_lower.contains("international") { return Some("970441"); }
    
    // VikkiBank - Ngân hàng số Vikki (check AFTER VIB to avoid false match)
    if name_lower.contains("vikkibank") || 
       name_lower.contains("vikki bank") || 
       name_lower.contains("ngân hàng số vikki") ||
       name_lower == "vikki" { 
        return Some("970461"); 
    }
    
    // VietABank - Ngân hàng TMCP Việt Á
    if name_lower.contains("vietabank") || name_lower.contains("vieta bank") || name_lower.contains("việt á") || name_lower.contains("viet a") || name_lower.contains("vab") { return Some("970427"); }
    
    // VietBank - Ngân hàng TMCP Việt Nam Thương Tín
    if name_lower.contains("vietbank") || name_lower.contains("viet bank") || name_lower.contains("thương tín") || name_lower.contains("thuong tin") { return Some("970433"); }
    
    // VietinBank - Ngân hàng TMCP Công Thương Việt Nam
    if name_lower.contains("vietinbank") || name_lower.contains("vietin bank") || name_lower.contains("công thương việt nam") || name_lower.contains("cong thuong viet nam") || name_lower.contains("cti") || name_lower.contains("icb") { return Some("970415"); }
    
    // VPBank - Ngân hàng TMCP Việt Nam Thịnh Vượng
    if name_lower.contains("vp bank") || name_lower.contains("vpbank") || name_lower.contains("việt nam thịnh vượng") || name_lower.contains("viet nam thinh vuong") || name_lower.contains("prosperity") { return Some("970432"); }
    
    // VRB - Ngân hàng Liên doanh Việt - Nga
    if name_lower.contains("vrb") || name_lower.contains("liên doanh") || name_lower.contains("lien doanh") || name_lower.contains("việt nga") || name_lower.contains("viet nga") { return Some("970421"); }
    
    // Woori - Ngân hàng TNHH MTV Woori Việt Nam
    if name_lower.contains("woori") { return Some("970457"); }
    
    None
}

fn generate_vietqr_url(
    bank_name: Option<&str>,
    account_no: Option<&str>,
    account_name: Option<&str>,
    amount: Option<&str>,
    message: Option<&str>
) -> Option<String> {
    let bank_name = bank_name?;
    let account_no = account_no?;
    
    println!("[DEBUG] Generating VietQR for bank: '{}', account: '{}'", bank_name, account_no);
    
    let bin = get_bank_bin(bank_name);
    if bin.is_none() {
        println!("[DEBUG] Failed to find BIN code for bank: '{}'", bank_name);
        return None;
    }
    let bin = bin.unwrap();
    println!("[DEBUG] Found BIN code: {}", bin);
    
    // VietQR API format: https://img.vietqr.io/image/{BIN}-{ACCOUNT_NO}-{TEMPLATE}.jpg
    let mut url = format!("https://img.vietqr.io/image/{}-{}-compact2.jpg", bin, account_no);
    
    let mut params = Vec::new();
    
    // Add amount if available (in VND)
    if let Some(amt) = amount {
        // Convert USDT to VND if needed (amount might be in USDT)
        let amt_clean = amt.replace(",", "").replace(".", "");
        if let Ok(num) = amt_clean.parse::<f64>() {
            // If amount < 1000, assume it's USDT and convert (rough estimate 27000 VND/USDT)
            let vnd_amount = if num < 1000.0 { (num * 27000.0) as i64 } else { num as i64 };
            params.push(format!("amount={}", vnd_amount));
        }
    }
    
    // KHÔNG thêm nội dung chuyển khoản - để ngân hàng tự động tạo nội dung mặc định
    // message và account_name sẽ KHÔNG được thêm vào QR
    
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    
    Some(url)
}

async fn handle_payment_detail(
    AxumState(state): AxumState<HttpAppState>,
    _headers: HeaderMap,
    Json(request): Json<ExtensionRequest>
) -> Result<ResponseJson<ApiResponse>, StatusCode> {
    println!("[DEBUG] Received payment detail request: {:?}", request.request_type);
    println!("[DEBUG] Request data: {:?}", request.data);
    
    // Only handle expected message type
    if request.request_type != "PAYMENT_DETAIL" {
        println!("[DEBUG] Unsupported request type: {}", request.request_type);
        return Ok(ResponseJson(ApiResponse {
            success: false,
            message: "Unsupported type".into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }));
    }

    // Extract fields
    let order_number = request
        .data
        .get("orderNumber")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    println!("[DEBUG] Extracted order_number: {}", order_number);

    if order_number.is_empty() {
        println!("[DEBUG] Order number is empty!");
        return Ok(ResponseJson(ApiResponse {
            success: false,
            message: "Missing orderNumber".into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }));
    }

    let get_str = |key: &str| -> Option<String> {
        request
            .data
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let account_name = get_str("accountName");
    let account_no = get_str("accountNo");
    let bank_name = get_str("bankName");
    let sub_bank = get_str("subBank");
    let mut qr_code_url = get_str("qrCodeUrl");
    
    // Clean and format amount
    let amount = get_str("amount").map(|s| {
        let cleaned = s.trim();
        if let Ok(num) = cleaned.parse::<f64>() {
            // Format as integer if it's a whole number, otherwise keep 2 decimals
            if num.fract() == 0.0 {
                format!("{:.0}", num)
            } else {
                format!("{:.2}", num)
            }
        } else {
            cleaned.to_string()
        }
    });
    
    let transfer_content = get_str("transferContent");
    let suggested_transfer_content = get_str("suggestedTransferContent");

    println!("[DEBUG] === EXTRACTED FIELDS FROM REQUEST ===");
    println!("  - order_number: {:?}", order_number);
    println!("  - account_name: {:?}", account_name);
    println!("  - account_no: {:?}", account_no);
    println!("  - bank_name: {:?}", bank_name);
    println!("  - sub_bank: {:?}", sub_bank);
    println!("  - amount: {:?}", amount);
    println!("  - transfer_content: {:?}", transfer_content);
    println!("  - suggested_transfer_content: {:?}", suggested_transfer_content);
    println!("  - qr_code_url: {:?}", qr_code_url.as_ref().map(|s| if s.len() > 50 { "present (truncated)" } else { s.as_str() }));
    println!("========================================");

    // ALWAYS generate VietQR from backend (ignore QR from extension/Binance)
    if account_no.is_some() && bank_name.is_some() {
        println!("[DEBUG] Generating VietQR from backend...");
        qr_code_url = generate_vietqr_url(
            bank_name.as_deref(),
            account_no.as_deref(),
            account_name.as_deref(),
            amount.as_deref(),
            transfer_content.as_deref().or(suggested_transfer_content.as_deref())
        );
        if qr_code_url.is_some() {
            println!("[DEBUG] Successfully generated VietQR URL");
        } else {
            println!("[DEBUG] Failed to generate VietQR (bank not supported or missing info)");
        }
    }

    // Persist to DB
    let pool = state.order_repo.pool();
    if let Err(_e) = sqlx::query(r#"
        INSERT OR REPLACE INTO order_payment_detail 
        (order_number, account_name, account_no, bank_name, sub_bank, qr_code_url, amount, transfer_content, suggested_transfer_content, captured_at, purge_after)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#)
        .bind(&order_number)
        .bind(account_name)
        .bind(account_no)
        .bind(bank_name)
        .bind(sub_bank)
        .bind(qr_code_url)
        .bind(amount)
        .bind(transfer_content)
        .bind(suggested_transfer_content)
        .bind(chrono::Utc::now().timestamp_millis())
        .bind(chrono::Utc::now().timestamp_millis() + 24*60*60*1000)
        .execute(pool)
        .await
    {
        println!("[DEBUG] DB error while saving payment detail: {:?}", _e);
        return Ok(ResponseJson(ApiResponse {
            success: false,
            message: "DB error while saving payment detail".into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }));
    }

    println!("[DEBUG] Payment detail saved successfully for order: {}", order_number);

    // Notify UI to reload
    let emit_result = state.handle.emit(
        "orders-updated",
        &serde_json::json!({"source":"extension","orderNumber": order_number}),
    );
    
    println!("[DEBUG] Event emission result: {:?}", emit_result);

    Ok(ResponseJson(ApiResponse {
        success: true,
        message: "Payment detail processed".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

// Health check handler
async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().timestamp_millis()
    })))
}

async fn start_http_server(state: HttpAppState) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/payment-detail", post(handle_payment_detail))
        .with_state(state)
        .layer(cors);
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:1425")
        .await
        .expect("Failed to bind HTTP server");
    
    println!("[HTTP Server] Listening on http://127.0.0.1:1425");
    let _ = axum::serve(listener, app).await;
}

#[tauri::command]
async fn fetch_payment_from_localstorage(app: tauri::AppHandle, order_number: String) -> Result<String, String> {
    println!("[DEBUG] Attempting to fetch payment from browser localStorage for order: {}", order_number);
    
    // Execute JavaScript in the webview to read localStorage
    // Note: This only works if there's an open webview with Binance domain
    let script = format!(
        r#"
        (function() {{
            try {{
                const key = 'p2p_payment_{}';
                const data = localStorage.getItem(key);
                return data || null;
            }} catch (e) {{
                return null;
            }}
        }})()
        "#,
        order_number
    );
    
    // Try to execute in all windows
    if let Some(window) = app.get_webview_window("main") {
        match window.eval(&script) {
            Ok(_) => {
                println!("[DEBUG] Successfully executed localStorage fetch script");
                Ok("Script executed".to_string())
            },
            Err(e) => {
                println!("[DEBUG] Failed to execute script: {}", e);
                Err(format!("Failed to execute script: {}", e))
            }
        }
    } else {
        println!("[DEBUG] No webview window found");
        Err("No webview window available".to_string())
    }
}

#[tauri::command]
async fn get_order_payment_detail(state: State<'_, AppCtx>, order_number: String) -> Result<Option<serde_json::Value>, String> {
    use sqlx::Row;
    println!("[DEBUG] get_order_payment_detail called for order: {}", order_number);
    let pool = state.order_repo.pool();
    
    let row = sqlx::query(r#"
        SELECT account_name, account_no, bank_name, sub_bank, qr_code_url, amount, transfer_content, suggested_transfer_content, captured_at
        FROM order_payment_detail 
        WHERE order_number = ?
    "#)
        .bind(&order_number)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    
    if let Some(row) = row {
        println!("[DEBUG] Found payment detail in database for order: {}", order_number);
        Ok(Some(serde_json::json!({
            "account_name": row.get::<Option<String>, _>("account_name"),
            "account_no": row.get::<Option<String>, _>("account_no"),
            "bank_name": row.get::<Option<String>, _>("bank_name"),
            "sub_bank": row.get::<Option<String>, _>("sub_bank"),
            "qr_code_url": row.get::<Option<String>, _>("qr_code_url"),
            "amount": row.get::<Option<String>, _>("amount"),
            "transfer_content": row.get::<Option<String>, _>("transfer_content"),
            "suggested_transfer_content": row.get::<Option<String>, _>("suggested_transfer_content"),
            "captured_at": row.get::<Option<i64>, _>("captured_at")
        })))
    } else {
        println!("[DEBUG] No payment detail found in database for order: {}", order_number);
        // Note: Frontend can call fetch_payment_from_localstorage as fallback
        Ok(None)
    }
}

#[tauri::command]
async fn list_recent_payment_details(state: State<'_, AppCtx>) -> Result<serde_json::Value, String> {
    use sqlx::Row;
    let pool = state.order_repo.pool();
    
    println!("[DEBUG] Listing recent payment details from database...");
    
    let rows = sqlx::query(r#"
        SELECT order_number, bank_name, account_no, 
               CASE WHEN qr_code_url IS NOT NULL THEN 'YES' ELSE 'NO' END as has_qr,
               created_at
        FROM order_payment_detail 
        ORDER BY created_at DESC 
        LIMIT 20
    "#)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let mut payments = Vec::new();
    for row in rows {
        payments.push(serde_json::json!({
            "order_number": row.get::<String, _>("order_number"),
            "bank_name": row.get::<Option<String>, _>("bank_name"),
            "account_no": row.get::<Option<String>, _>("account_no"),
            "has_qr": row.get::<String, _>("has_qr"),
            "created_at": row.get::<i64, _>("created_at")
        }));
    }
    
    println!("[DEBUG] Found {} payment details in database", payments.len());
    
    Ok(serde_json::json!({
        "count": payments.len(),
        "payments": payments
    }))
}

#[tauri::command]
async fn get_db_stats(state: State<'_, AppCtx>) -> Result<serde_json::Value, String> {
    use sqlx::Row;
    let pool = state.order_repo.pool();
    
    // Đếm tổng số lệnh
    let total_count = sqlx::query("SELECT COUNT(*) as count FROM orders")
        .fetch_one(pool).await.map_err(|e| e.to_string())?
        .get::<i64, _>("count");
    
    // Đếm theo trade_type 
    let buy_count = sqlx::query("SELECT COUNT(*) as count FROM orders WHERE trade_type = 'BUY'")
        .fetch_one(pool).await.map_err(|e| e.to_string())?
        .get::<i64, _>("count");
        
    let sell_count = sqlx::query("SELECT COUNT(*) as count FROM orders WHERE trade_type = 'SELL'")
        .fetch_one(pool).await.map_err(|e| e.to_string())?
        .get::<i64, _>("count");
    
    // Đếm theo status
    let status_rows = sqlx::query("SELECT order_status_code, COUNT(*) as count FROM orders GROUP BY order_status_code")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    
    let mut status_stats = serde_json::Map::new();
    for row in status_rows {
        let code: i64 = row.get("order_status_code");
        let count: i64 = row.get("count");
        status_stats.insert(code.to_string(), serde_json::Value::Number(count.into()));
    }
    
    Ok(serde_json::json!({
        "total": total_count,
        "buy_count": buy_count, 
        "sell_count": sell_count,
        "status_breakdown": status_stats
    }))
}

#[tokio::main]
async fn main() {
    // Use AppData directory for database (fixed location, works across machines)
    let app_data_dir = dirs::data_local_dir()
        .expect("Failed to get AppData directory")
        .join("BinanceP2PManager");
    
    // Create directory if it doesn't exist
    std::fs::create_dir_all(&app_data_dir)
        .expect("Failed to create app data directory");
    
    let db_path = app_data_dir.join("p2p_app.db");
    println!("[INIT] Database path: {:?}", db_path);
    
    let db = Arc::new(Db::init(db_path.to_str().unwrap()).await.expect("DB init failed"));
    let crypto = CryptoCtx::new_dummy();
    let creds_repo = Arc::new(CredentialsRepo::new(db.pool().clone(), crypto));
    
    // Seed API credentials from local file if present (ignored by git)
    // Still look for seed file in exe directory for development
    let exe_dir = std::env::current_exe()
        .expect("Failed to get exe path")
        .parent()
        .expect("Failed to get exe directory")
        .to_path_buf();
    
    let seed_path = exe_dir.parent().unwrap_or(&exe_dir).join("db/seed_credentials.json");
    if let Ok(data) = fs::read_to_string(&seed_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
            let label = v.get("label").and_then(|x| x.as_str()).unwrap_or("seed");
            if let (Some(api), Some(secret)) = (v.get("api").and_then(|x| x.as_str()), v.get("api_secret").and_then(|x| x.as_str())) {
                let _ = creds_repo.store(label, api, secret).await;
                // prevent reseeding on every run
                let applied_path = exe_dir.parent().unwrap_or(&exe_dir).join("db/seed_credentials.applied.json");
                let _ = fs::rename(&seed_path, &applied_path);
            }
        }
    }
    
    // Load status label mapping from db/stage_map.json (exe directory)
    let stage_map_path = exe_dir.parent().unwrap_or(&exe_dir).join("db/stage_map.json");
    let stage_map = Arc::new(orders::stage_map::StageMap::load_from(stage_map_path.to_str().unwrap()));
    let order_repo = Arc::new(OrderRepo::new(db.pool().clone(), stage_map.clone()));

    let api_client = {
        let mut opt = None;
        if let Ok(Some((k,s))) = creds_repo.latest().await { opt = Some(C2CApiClient::new(k, s)); }
        Arc::new(RwLock::new(opt))
    };

    let app_ctx = AppCtx { order_repo: order_repo.clone(), creds_repo: creds_repo.clone(), api_client: api_client.clone() };

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            let repo_sched = order_repo.clone();
            let api = api_client.clone();
            
            // Start background scheduler
            tauri::async_runtime::spawn(async move {
                start_scheduler(handle, repo_sched, api).await;
            });
            
            // Start HTTP server for extension communication
            let http_state = HttpAppState { order_repo: order_repo.clone(), handle: app.handle().clone() };
            tauri::async_runtime::spawn(async move {
                start_http_server(http_state).await;
            });
            
            Ok(())
        })
        .manage(app_ctx)
        .invoke_handler(tauri::generate_handler![
            store_api_credentials,
            check_api_credentials,
            get_saved_credentials,
            test_api_credentials,
            force_initial_sync,
            force_sync_recent,
            list_orders_from_db,
            get_db_stats,
            list_recent_payment_details,
            get_order_payment_detail,
            save_payment_detail_from_extension,
            cleanup_old_payment_details,
            fetch_payment_from_localstorage,
            clear_all_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn start_scheduler(handle: tauri::AppHandle, repo: Arc<OrderRepo>, api_client: Arc<RwLock<Option<C2CApiClient>>>) {
    use std::time::Duration;
    use tokio::time::{interval, MissedTickBehavior};
    loop {
    if let Some(client) = { api_client.read().await.clone() } {
            let engine = SyncEngine::new(&client, &repo);
            // 60s incremental sync
            let mut inc = interval(Duration::from_secs(60));
            inc.set_missed_tick_behavior(MissedTickBehavior::Delay);
            // 15s active poll
            let mut poll = interval(Duration::from_secs(15));
            poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
            // 5 minute cleanup interval
            let mut cleanup = interval(Duration::from_secs(300));
            cleanup.set_missed_tick_behavior(MissedTickBehavior::Delay);
            
            // ✅ Do NOT auto force_initial_sync here!
            // Let user choose sync days via UI (Settings → Đồng bộ từ Binance)
            // Just do incremental sync to catch any new orders since last sync
            let _ = engine.incremental_sync().await;
            let _ = handle.emit("orders-updated", &serde_json::json!({"source":"initial"}));
            
            loop {
                tokio::select! {
                    _ = inc.tick() => {
                        let _ = engine.incremental_sync().await;
                        let _ = handle.emit("orders-updated", &serde_json::json!({"source":"incremental"}));
                    }
                    _ = poll.tick() => {
                        let _ = engine.active_poll().await;
                        let _ = handle.emit("orders-updated", &serde_json::json!({"source":"poll"}));
                    }
                    _ = cleanup.tick() => {
                        // Auto-cleanup old payment details
                        let pool = repo.pool();
                        if let Ok(_res) = sqlx::query(r#"
                            DELETE FROM order_payment_detail 
                            WHERE order_number NOT IN (
                                SELECT order_number FROM orders 
                                WHERE status_code IN (1, 2, 3)
                            )
                            OR purge_after < ?
                        "#)
                        .bind(chrono::Utc::now().timestamp_millis())
                        .execute(pool)
                        .await {
                            // Cleanup completed silently
                        }
                    }
                }
                // break if api client removed
                if api_client.read().await.is_none() { break; }
            }
        } else {
            // Wait and retry if no credentials yet
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}