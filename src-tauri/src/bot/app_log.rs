use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn log(msg: impl AsRef<str>) {
    let msg = msg.as_ref().to_string();
    if let Some(app) = APP.get() {
        let _ = app.emit("bot-log", msg.clone());
    }
    tracing::info!(target: "p2p_qr::bot", "{msg}");
}

/// Rút gọn mã lệnh trong log: `…58560`
pub fn oid(order_no: &str) -> String {
    if order_no.len() <= 8 {
        order_no.to_string()
    } else {
        format!("…{}", &order_no[order_no.len() - 6..])
    }
}
