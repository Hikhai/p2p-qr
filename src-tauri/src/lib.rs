use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::State;

// Trạng thái chia sẻ để lưu tối đa 200 message WebSocket (string)
#[derive(Clone, Debug)]
struct WsState {
    buffer: Arc<Mutex<Vec<String>>>
}

impl WsState {
    fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new()))
        }
    }
}

// Command để frontend gọi
#[tauri::command]
async fn get_raw_messages(state: State<'_, WsState>) -> Result<Vec<String>, String> {
    let buf = state.buffer.lock().await;
    let len = buf.len();
    Ok(buf.iter().skip(len.saturating_sub(10)).cloned().collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ws_state = WsState::new();

    // Spawn WebSocket server trong setup
    let ws_state_for_task = ws_state.clone();
    tauri::Builder::default()
        .setup(move |_app| {
            // Spawn task async using Tauri's async runtime
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_ws_server("127.0.0.1:8123", ws_state_for_task).await {
                    eprintln!("[WS] Server crashed: {e:?}");
                }
            });
            println!("[INIT] App started. WS listening on ws://127.0.0.1:8123");
            Ok(())
        })
        .manage(ws_state)
        .invoke_handler(tauri::generate_handler![get_raw_messages])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------------- WebSocket Server ----------------
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures::StreamExt;
use anyhow::Result;

async fn start_ws_server(addr: &str, shared: WsState) -> Result<()> {
    println!("[WS] Binding {addr}");
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let peer_state = shared.clone();
        tauri::async_runtime::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[WS] Handshake error: {e}");
                    return;
                }
            };
            println!("[WS] Client connected");
            let (mut _write, mut read) = ws_stream.split();
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(m) if m.is_text() => {
                        let txt = m.into_text().unwrap();
                        let mut g = peer_state.buffer.lock().await;
                        g.push(txt);
                        if g.len() > 200 { g.drain(0..100); }
                        println!("[WS] Received (total stored: {})", g.len());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[WS] Read error: {e}");
                        break;
                    }
                }
            }
            println!("[WS] Client disconnected");
        });
    }
}
