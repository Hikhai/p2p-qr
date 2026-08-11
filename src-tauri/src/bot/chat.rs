use crate::bot::app_log;
use crate::bot::binance::BotBinanceClient;
use anyhow::{anyhow, Result};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

/// Quản lý WebSocket chat P2P Binance.
pub struct ChatManager {
    out_tx: mpsc::UnboundedSender<String>,
    connected: Arc<AtomicBool>,
    pub event_notify: Arc<Notify>,
}

impl ChatManager {
    pub fn start(client: Arc<BotBinanceClient>) -> Arc<ChatManager> {
        let (out_tx, out_rx) = mpsc::unbounded_channel::<String>();
        let mgr = Arc::new(ChatManager {
            out_tx,
            connected: Arc::new(AtomicBool::new(false)),
            event_notify: Arc::new(Notify::new()),
        });
        let mgr_clone = mgr.clone();
        tokio::spawn(async move {
            run_loop(client, out_rx, mgr_clone).await;
        });
        mgr
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn send_text(&self, order_no: &str, content: &str, nick: Option<&str>) -> Result<()> {
        let mut frame = json!({
            "type": "text",
            "uuid": Uuid::new_v4().to_string(),
            "orderNo": order_no,
            "content": content,
            "self": true,
            "clientType": "web",
            "createTime": Utc::now().timestamp_millis(),
            "sendStatus": 0
        });
        if let Some(n) = nick.filter(|s| !s.is_empty()) {
            frame
                .as_object_mut()
                .unwrap()
                .insert("fromNickName".into(), json!(n));
        }
        self.send_frame(frame)
    }

    /// Frame ảnh khớp mẫu Binance thật:
    /// `imageType: "IMAGE"`, có `thumbnailUrl`, `width`, `height`, `fromNickName`.
    pub fn send_image(
        &self,
        order_no: &str,
        image_url: &str,
        width: u32,
        height: u32,
        nick: Option<&str>,
    ) -> Result<()> {
        let mut frame = json!({
            "type": "image",
            "uuid": Uuid::new_v4().to_string(),
            "orderNo": order_no,
            "imageUrl": image_url,
            "thumbnailUrl": image_url,
            "imageType": "IMAGE",
            "width": width,
            "height": height,
            "self": true,
            "clientType": "web",
            "createTime": Utc::now().timestamp_millis(),
            "sendStatus": 0
        });
        if let Some(n) = nick.filter(|s| !s.is_empty()) {
            frame
                .as_object_mut()
                .unwrap()
                .insert("fromNickName".into(), json!(n));
        }
        self.send_raw(frame.to_string())
    }

    fn send_frame(&self, frame: serde_json::Value) -> Result<()> {
        self.send_raw(frame.to_string())
    }

    fn send_raw(&self, raw: String) -> Result<()> {
        if !self.is_connected() {
            return Err(anyhow!("Chat WebSocket chưa kết nối"));
        }
        self.out_tx
            .send(raw)
            .map_err(|_| anyhow!("Kênh gửi WebSocket đã đóng"))
    }
}

async fn run_loop(
    client: Arc<BotBinanceClient>,
    mut out_rx: mpsc::UnboundedReceiver<String>,
    mgr: Arc<ChatManager>,
) {
    let mut backoff_secs: u64 = 5;
    loop {
        let cred = match client.retrieve_chat_credential().await {
            Ok(c) => c,
            Err(e) => {
                app_log::log(format!("[CHAT] credential lỗi ({e}) — lại sau {backoff_secs}s"));
                tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
                continue;
            }
        };
        let (wss_url, listen_key, listen_token) = cred;
        let ws_url = format!(
            "{}/{}?token={}&clientType=web",
            wss_url.trim_end_matches('/'),
            listen_key,
            listen_token
        );

        let ws = match connect_async(&ws_url).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                app_log::log(format!("[CHAT] WS lỗi ({e}) — lại sau {backoff_secs}s"));
                tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
                continue;
            }
        };

        app_log::log("[CHAT] đã kết nối");
        mgr.connected.store(true, Ordering::Relaxed);
        backoff_secs = 5;

        let (mut write, mut read) = ws.split();
        let mut ping = tokio::time::interval(std::time::Duration::from_secs(30));
        ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(m)) => {
                            if m.is_text() {
                                if let Ok(txt) = m.into_text() {
                                    handle_incoming(&txt, &mgr);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            app_log::log(format!("[CHAT] đọc lỗi: {e}"));
                            break;
                        }
                        None => {
                            app_log::log("[CHAT] WS đóng");
                            break;
                        }
                    }
                }
                out = out_rx.recv() => {
                    match out {
                        Some(frame) => {
                            if let Err(e) = write.send(Message::Text(frame)).await {
                                app_log::log(format!("[CHAT] gửi lỗi: {e}"));
                                break;
                            }
                        }
                        None => return,
                    }
                }
                _ = ping.tick() => {
                    if write.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                }
            }
        }

        mgr.connected.store(false, Ordering::Relaxed);
        app_log::log(format!("[CHAT] mất kết nối — lại sau {backoff_secs}s"));
        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
    }
}

fn handle_incoming(raw: &str, mgr: &ChatManager) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return;
    };
    let order_no = v.get("orderNo").and_then(|x| x.as_str()).unwrap_or("");
    if order_no.is_empty() {
        return;
    }
    let is_self = v.get("self").and_then(|x| x.as_bool()).unwrap_or(false);
    if is_self {
        return;
    }
    let msg_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("?");
    app_log::log(format!(
        "[CHAT] {} {} → quét",
        msg_type,
        app_log::oid(order_no)
    ));
    mgr.event_notify.notify_one();
}
