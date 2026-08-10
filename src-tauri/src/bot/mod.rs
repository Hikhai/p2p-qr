mod app_log;
mod binance;
mod chat;
mod config;
mod session;

pub use config::BotConfig;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};

pub struct BotRuntime {
    running: AtomicBool,
    stop: Mutex<Option<Arc<AtomicBool>>>,
}

impl Default for BotRuntime {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            stop: Mutex::new(None),
        }
    }
}

pub fn init_logging(app: &AppHandle) {
    app_log::init(app.clone());
}

#[tauri::command]
pub fn get_bot_config() -> Result<BotConfig, String> {
    config::load_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_bot_config(cfg: BotConfig) -> Result<(), String> {
    config::validate_config(&cfg).map_err(|e| e.to_string())?;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    app_log::log("[UI] đã lưu cấu hình");
    Ok(())
}

#[tauri::command]
pub fn get_bot_status(state: State<'_, BotRuntime>) -> String {
    if state.running.load(Ordering::Relaxed) {
        "running".into()
    } else {
        "idle".into()
    }
}

#[tauri::command]
pub async fn start_bot(
    app: AppHandle,
    bot_state: State<'_, BotRuntime>,
    ctx: State<'_, crate::AppCtx>,
    cfg: BotConfig,
) -> Result<(), String> {
    if bot_state.running.load(Ordering::Relaxed) {
        return Err("Bot đang chạy".into());
    }
    config::validate_config(&cfg).map_err(|e| e.to_string())?;

    let (api_key, api_secret) = ctx
        .creds_repo
        .load()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Chưa lưu API credentials — vào Cài đặt để lưu API key trước".to_string())?;

    config::save_config(&cfg).map_err(|e| e.to_string())?;

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut guard = bot_state.stop.lock().map_err(|e| e.to_string())?;
        *guard = Some(stop.clone());
    }
    bot_state.running.store(true, Ordering::Relaxed);
    let _ = app.emit("bot-status", "running");

    let state_path = config::state_path();
    let app_for_done = app.clone();

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                app_log::log(format!("Không tạo được runtime: {e}"));
                if let Some(st) = app_for_done.try_state::<BotRuntime>() {
                    st.running.store(false, Ordering::Relaxed);
                    if let Ok(mut g) = st.stop.lock() {
                        *g = None;
                    }
                }
                let _ = app_for_done.emit("bot-status", "idle");
                return;
            }
        };

        let result = rt.block_on(session::run_session(
            cfg,
            api_key,
            api_secret,
            state_path,
            stop,
        ));
        match result {
            Ok(()) => app_log::log("[BOT] dừng"),
            Err(e) => app_log::log(format!("[BOT] dừng lỗi: {e}")),
        }

        if let Some(st) = app_for_done.try_state::<BotRuntime>() {
            st.running.store(false, Ordering::Relaxed);
            if let Ok(mut g) = st.stop.lock() {
                *g = None;
            }
        }
        let _ = app_for_done.emit("bot-status", "idle");
    });

    app_log::log("[UI] start");
    Ok(())
}

#[tauri::command]
pub fn stop_bot(app: AppHandle, state: State<'_, BotRuntime>) -> Result<(), String> {
    if !state.running.load(Ordering::Relaxed) {
        return Err("Bot chưa chạy".into());
    }
    if let Ok(guard) = state.stop.lock() {
        if let Some(flag) = guard.as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
    }
    let _ = app.emit("bot-status", "stopping");
    app_log::log("[UI] stop");
    Ok(())
}
