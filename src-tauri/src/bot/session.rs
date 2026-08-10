use crate::bot::app_log;
use crate::bot::binance::BotBinanceClient;
use crate::bot::chat::ChatManager;
use crate::bot::config::BotConfig;
use crate::vietqr;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const MAX_ATTEMPTS: u32 = 5;
const RETRY_COOLDOWN_MS: i64 = 45_000;

struct PreparedChatImage {
    url: String,
    width: u32,
    height: u32,
}

/// Trạng thái gửi tin theo từng lệnh — welcome (tin1+QR) và complete (tin3) tách biệt.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderState {
    #[serde(default)]
    welcome_sent: bool,
    #[serde(default)]
    complete_sent: bool,
    #[serde(default)]
    welcome_attempts: u32,
    #[serde(default)]
    complete_attempts: u32,
    #[serde(default)]
    welcome_sending: bool,
    #[serde(default)]
    complete_sending: bool,
    #[serde(default)]
    last_welcome_ms: Option<i64>,
    #[serde(default)]
    last_complete_ms: Option<i64>,
    /// Status lần quét trước — tin 3 chỉ gửi khi chuyển → COMPLETED lúc bot đang chạy.
    #[serde(default)]
    last_status: Option<i64>,
    /// Đã từng thấy BUYER_PAYED/DISTRIBUTING — bắt buộc trước khi gửi tin 3.
    #[serde(default)]
    saw_buyer_paid: bool,
    #[serde(default)]
    error: Option<String>,
    /// Bản cũ: "sent" | "failed" | "sending" — chỉ dùng để migrate.
    #[serde(default)]
    status: Option<String>,
}

pub struct Bot {
    cfg: BotConfig,
    client: Arc<BotBinanceClient>,
    chat: Arc<ChatManager>,
    state_path: PathBuf,
    state: HashMap<String, OrderState>,
    last_time_sync_ms: i64,
}

impl Bot {
    pub fn new(
        cfg: BotConfig,
        client: Arc<BotBinanceClient>,
        chat: Arc<ChatManager>,
        state_path: PathBuf,
    ) -> Self {
        let mut state: HashMap<String, OrderState> = std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        // Migrate state cũ → welcome_sent (KHÔNG reset để gửi lại).
        let mut migrated = 0u32;
        for st in state.values_mut() {
            if let Some(legacy) = st.status.take() {
                match legacy.as_str() {
                    "sent" => {
                        if !st.welcome_sent {
                            st.welcome_sent = true;
                            migrated += 1;
                        }
                    }
                    "sending" => {
                        st.welcome_sending = false;
                        st.complete_sending = false;
                    }
                    _ => {}
                }
            }
            st.welcome_sending = false;
            st.complete_sending = false;
        }
        if migrated > 0 {
            app_log::log(format!("[INIT] migrate {migrated} lệnh (đã welcome)"));
        }

        let now = Utc::now().timestamp_millis();
        let bot = Self {
            cfg,
            client,
            chat,
            state_path,
            state,
            last_time_sync_ms: now,
        };
        bot.save_state_ref();
        bot
    }

    fn save_state_ref(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.state) {
            let _ = std::fs::write(&self.state_path, json);
        }
    }

    pub async fn run(&mut self, stop: Arc<AtomicBool>) -> Result<()> {
        for _ in 0..40 {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            if self.chat.is_connected() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        if !self.chat.is_connected() {
            app_log::log("[BOT] chat chưa sẵn sàng — sẽ thử lại");
        }

        let poll = std::time::Duration::from_secs(self.cfg.poll_interval_secs.max(5));
        let mut tick = tokio::time::interval(poll);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        app_log::log(format!("[BOT] chạy · poll {}s", poll.as_secs()));

        if let Err(e) = self.scan_and_process().await {
            app_log::log(format!("[BOT] lỗi quét: {e}"));
        }

        loop {
            if stop.load(Ordering::Relaxed) {
                app_log::log("[BOT] dừng");
                break;
            }
            tokio::select! {
                _ = tick.tick() => {}
                _ = self.chat.event_notify.notified() => {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                }
                _ = wait_until_stopped(stop.clone()) => {
                    app_log::log("[BOT] dừng");
                    break;
                }
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let now = Utc::now().timestamp_millis();
            if now - self.last_time_sync_ms > 60 * 60 * 1000 {
                if self.client.sync_time().await.is_ok() {
                    self.last_time_sync_ms = now;
                }
            }
            if let Err(e) = self.scan_and_process().await {
                app_log::log(format!("[BOT] lỗi quét: {e}"));
            }
        }
        Ok(())
    }

    async fn scan_and_process(&mut self) -> Result<()> {
        let window_ms = self.cfg.order_max_age_minutes.max(5) * 60 * 1000;
        let orders = self.client.list_recent_sell_orders(window_ms).await?;
        let now = Utc::now().timestamp_millis();

        for order in &orders {
            let order_no = order
                .get("orderNumber")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if order_no.is_empty() {
                continue;
            }
            let create_time = get_i64(order, "createTime");
            if create_time > 0 && now.saturating_sub(create_time) > window_ms {
                continue;
            }

            let list_code = order_status_code(order);

            // ── Tin 1 + QR: lệnh đang chờ thanh toán ──
            if is_pending_payment_code(list_code) {
                self.try_send_welcome(order_no, order, now).await;
                // Trường hợp trước đó chỉ gửi được tin text (CDN lỗi) — bổ sung QR.
                self.try_resend_missing_qr(order_no, order, now).await;
            }

            // Đánh dấu đã thấy giai đoạn buyer đã CK (trước khi seller mở khóa).
            if is_buyer_paid_code(list_code) || is_distributing_code(list_code) {
                let st = self.state.entry(order_no.to_string()).or_default();
                st.saw_buyer_paid = true;
            }

            // ── Tin 3: history báo COMPLETED → xác minh bằng detail + checkIfCanReleaseCoin ──
            let effective_code = if is_completed_code(list_code) {
                self.try_send_complete(order_no, order, now)
                    .await
                    .or(list_code)
            } else {
                list_code
            };

            let st = self.state.entry(order_no.to_string()).or_default();
            st.last_status = effective_code;
            self.save_state();
        }
        Ok(())
    }

    async fn try_send_welcome(&mut self, order_no: &str, order: &Value, now: i64) {
        let st = self.state.entry(order_no.to_string()).or_default();
        if st.welcome_sent || st.welcome_sending {
            return;
        }
        if st.welcome_attempts >= MAX_ATTEMPTS {
            return;
        }
        if let Some(last) = st.last_welcome_ms {
            if now - last < RETRY_COOLDOWN_MS {
                return;
            }
        }

        let attempts = st.welcome_attempts + 1;
        st.welcome_sending = true;
        st.welcome_attempts = attempts;
        st.last_welcome_ms = Some(now);
        st.error = None;
        self.save_state();

        let oid = app_log::oid(order_no);
        app_log::log(format!(
            "[BOT] tin1+QR {oid} ({attempts}/{MAX_ATTEMPTS})"
        ));

        match self.send_welcome(order_no, order).await {
            Ok(()) => {
                app_log::log(format!("[BOT] ok tin1+QR {oid}"));
                let st = self.state.entry(order_no.to_string()).or_default();
                st.welcome_sent = true;
                st.welcome_sending = false;
                st.error = None;
            }
            Err(e) => {
                let msg = e.to_string();
                let soft = msg.contains("WebSocket chưa kết nối");
                app_log::log(format!("[BOT] lỗi tin1 {oid}: {msg}"));
                let st = self.state.entry(order_no.to_string()).or_default();
                st.welcome_sending = false;
                if soft {
                    st.welcome_attempts = attempts.saturating_sub(1);
                    st.last_welcome_ms =
                        Some(Utc::now().timestamp_millis() - RETRY_COOLDOWN_MS + 3_000);
                }
                st.error = Some(msg);
            }
        }
        self.save_state();
    }

    /// Trả về status “thật” để lưu state (2 = còn mở khóa / chưa xong).
    async fn try_send_complete(
        &mut self,
        order_no: &str,
        order: &Value,
        now: i64,
    ) -> Option<i64> {
        let st = self.state.entry(order_no.to_string()).or_default();
        // Chỉ gửi tin 3 nếu đã gửi welcome (tránh spam lệnh cũ khi bot start).
        if !st.welcome_sent || st.complete_sent || st.complete_sending {
            return None;
        }
        if self.cfg.instruction_message.trim().is_empty() {
            return None;
        }
        if st.complete_attempts >= MAX_ATTEMPTS {
            return None;
        }
        if let Some(last) = st.last_complete_ms {
            if now - last < RETRY_COOLDOWN_MS {
                return None;
            }
        }

        let oid = app_log::oid(order_no);

        // 1) Chi tiết lệnh — ưu tiên hơn list history (history đôi khi nhảy sớm sang COMPLETED).
        let detail = match self.client.get_order_detail(order_no).await {
            Ok(d) => d,
            Err(e) => {
                app_log::log(format!("[BOT] hoãn tin3 {oid}: detail {e}"));
                return None;
            }
        };
        let detail_code = order_status_code(&detail).or_else(|| order_status_code(order));
        if is_buyer_paid_code(detail_code) || is_distributing_code(detail_code) {
            let st = self.state.entry(order_no.to_string()).or_default();
            st.saw_buyer_paid = true;
        }
        if !is_completed_code(detail_code) {
            // Im lặng — tránh spam mỗi poll khi chưa xong.
            return detail_code;
        }

        // 2) Còn mở khóa được = giao dịch chưa xong (UI còn nút "Mở khóa nhanh").
        match self.client.can_release_coin(order_no).await {
            Ok(true) => {
                let st = self.state.entry(order_no.to_string()).or_default();
                st.saw_buyer_paid = true;
                return Some(2);
            }
            Ok(false) => {}
            Err(e) => {
                app_log::log(format!("[BOT] hoãn tin3 {oid}: release? {e}"));
                return None;
            }
        }

        let st = self.state.entry(order_no.to_string()).or_default();
        let attempts = st.complete_attempts + 1;
        st.complete_sending = true;
        st.complete_attempts = attempts;
        st.last_complete_ms = Some(now);
        st.saw_buyer_paid = true;
        self.save_state();

        app_log::log(format!("[BOT] tin3 {oid} ({attempts}/{MAX_ATTEMPTS})"));

        match self.send_complete(order_no, order).await {
            Ok(()) => {
                app_log::log(format!("[BOT] ok tin3 {oid}"));
                let st = self.state.entry(order_no.to_string()).or_default();
                st.complete_sent = true;
                st.complete_sending = false;
            }
            Err(e) => {
                let msg = e.to_string();
                let soft = msg.contains("WebSocket chưa kết nối");
                app_log::log(format!("[BOT] lỗi tin3 {oid}: {msg}"));
                let st = self.state.entry(order_no.to_string()).or_default();
                st.complete_sending = false;
                if soft {
                    st.complete_attempts = attempts.saturating_sub(1);
                    st.last_complete_ms =
                        Some(Utc::now().timestamp_millis() - RETRY_COOLDOWN_MS + 3_000);
                }
                st.error = Some(msg);
            }
        }
        self.save_state();
        Some(4)
    }

    /// Tin 1 (chào) + ảnh QR — gửi nhanh khi có lệnh chờ thanh toán.
    async fn send_welcome(&self, order_no: &str, order: &Value) -> Result<()> {
        if !self.chat.is_connected() {
            return Err(anyhow!("Chat WebSocket chưa kết nối, sẽ thử lại ở chu kỳ sau"));
        }

        let oid = app_log::oid(order_no);
        let detail = match self.client.get_order_detail(order_no).await {
            Ok(d) => Some(d),
            Err(e) => {
                app_log::log(format!("[BOT] {oid}: dùng TK dự phòng ({e})"));
                None
            }
        };
        let pay = detail.as_ref().map(extract_payment_info).unwrap_or_default();

        let bank_name = non_empty(&pay.bank_name).unwrap_or(self.cfg.bank_name.as_str());
        let account_no = non_empty(&pay.account_no).unwrap_or(self.cfg.account_no.as_str());
        let account_name = non_empty(&pay.account_name).unwrap_or(self.cfg.account_name.as_str());
        if bank_name.trim().is_empty() || account_no.trim().is_empty() {
            return Err(anyhow!(
                "Lệnh không có thông tin ngân hàng và chưa cấu hình tài khoản dự phòng"
            ));
        }

        let amount_vnd = pay
            .amount_vnd
            .or_else(|| parse_amount_vnd(order))
            .ok_or_else(|| anyhow!("Không đọc được số tiền (totalPrice) của lệnh"))?;

        let buyer_norm = normalize_person_name(&pay.buyer_name);
        let render = |template: &str| -> String {
            template
                .replace("{ma_lenh}", order_no)
                .replace("{so_tien}", &format_vnd(amount_vnd))
                .replace("{ten_nguoi_mua}", &buyer_norm)
        };

        let add_info = resolve_qr_add_info(&self.cfg.qr_transfer_content, &render);
        app_log::log(format!(
            "[BOT] {oid}: {} {} · {} VND · CK={}",
            bank_name.trim(),
            account_no.trim(),
            format_vnd(amount_vnd),
            add_info.as_deref().unwrap_or("(bank)")
        ));

        let qr_url = vietqr::image_url_with_account_name(
            bank_name,
            account_no,
            Some(amount_vnd),
            add_info.as_deref(),
            non_empty(account_name),
        )
        .ok_or_else(|| {
            anyhow!("Không tạo được VietQR cho ngân hàng '{bank_name}' / STK '{account_no}'")
        })?;

        // Upload QR trước để gửi tin chào + ảnh sát nhau.
        let prepared = self
            .prepare_chat_qr_image(order_no, &qr_url)
            .await
            .map_err(|e| anyhow!("Chuẩn bị ảnh QR thất bại: {e}"))?;

        let nick = self.resolve_from_nickname(order_no, order).await;
        let existing = self
            .client
            .list_chat_messages(order_no)
            .await
            .unwrap_or_default();
        let already_greeted = existing.iter().any(|m| {
            m.get("self").and_then(|s| s.as_bool()) == Some(true)
                && m.get("type").and_then(|t| t.as_str()) == Some("text")
        });

        if !already_greeted {
            self.chat
                .send_text(
                    order_no,
                    &render(&self.cfg.greeting_message),
                    nick.as_deref(),
                )
                .map_err(|e| anyhow!("Gửi tin chào thất bại: {e}"))?;
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        }

        self.chat
            .send_image(
                order_no,
                &prepared.url,
                prepared.width,
                prepared.height,
                nick.as_deref(),
            )
            .map_err(|e| anyhow!("Gửi ảnh QR thất bại: {e}"))?;

        // Xác nhận ảnh đã vào chat — tránh đánh dấu OK khi chỉ có tin text.
        for wait in [1800u64, 2000] {
            tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            if let Ok(msgs) = self.client.list_chat_messages(order_no).await {
                if BotBinanceClient::chat_has_image(&msgs, &prepared.url)
                    || BotBinanceClient::chat_has_self_image(&msgs)
                {
                    return Ok(());
                }
            }
        }
        Err(anyhow!(
            "Đã gửi frame ảnh nhưng chưa thấy trong chat — sẽ thử lại (CDN/URL)"
        ))
    }

    /// Welcome đã đánh dấu sent nhưng chat chưa có ảnh self → gửi lại chỉ QR.
    async fn try_resend_missing_qr(&mut self, order_no: &str, order: &Value, now: i64) {
        let st = self.state.get(order_no);
        let Some(st) = st else { return };
        if !st.welcome_sent || st.welcome_sending || st.complete_sent {
            return;
        }
        // Cooldown ngắn hơn welcome đầy đủ — chỉ bổ sung QR.
        if let Some(last) = st.last_welcome_ms {
            if now - last < 12_000 {
                return;
            }
        }

        let has_img = match self.client.list_chat_messages(order_no).await {
            Ok(msgs) => BotBinanceClient::chat_has_self_image(&msgs),
            Err(_) => return,
        };
        if has_img {
            return;
        }

        let oid = app_log::oid(order_no);
        app_log::log(format!("[BOT] gửi lại QR {oid}"));
        let st = self.state.entry(order_no.to_string()).or_default();
        st.welcome_sending = true;
        st.last_welcome_ms = Some(now);
        self.save_state();

        let result = self.send_qr_only(order_no, order).await;
        let st = self.state.entry(order_no.to_string()).or_default();
        st.welcome_sending = false;
        match result {
            Ok(()) => {
                app_log::log(format!("[BOT] ok QR {oid}"));
                st.error = None;
            }
            Err(e) => {
                app_log::log(format!("[BOT] lỗi QR {oid}: {e}"));
                st.error = Some(e.to_string());
            }
        }
        self.save_state();
    }

    async fn send_qr_only(&self, order_no: &str, order: &Value) -> Result<()> {
        if !self.chat.is_connected() {
            return Err(anyhow!("Chat WebSocket chưa kết nối"));
        }
        let detail = self.client.get_order_detail(order_no).await.ok();
        let pay = detail.as_ref().map(extract_payment_info).unwrap_or_default();
        let bank_name = non_empty(&pay.bank_name).unwrap_or(self.cfg.bank_name.as_str());
        let account_no = non_empty(&pay.account_no).unwrap_or(self.cfg.account_no.as_str());
        let account_name = non_empty(&pay.account_name).unwrap_or(self.cfg.account_name.as_str());
        if bank_name.trim().is_empty() || account_no.trim().is_empty() {
            return Err(anyhow!("Thiếu thông tin ngân hàng để tạo QR"));
        }
        let amount_vnd = pay
            .amount_vnd
            .or_else(|| parse_amount_vnd(order))
            .ok_or_else(|| anyhow!("Không đọc được số tiền"))?;
        let buyer_norm = normalize_person_name(&pay.buyer_name);
        let render = |template: &str| -> String {
            template
                .replace("{ma_lenh}", order_no)
                .replace("{so_tien}", &format_vnd(amount_vnd))
                .replace("{ten_nguoi_mua}", &buyer_norm)
        };
        let add_info = resolve_qr_add_info(&self.cfg.qr_transfer_content, &render);
        let qr_url = vietqr::image_url_with_account_name(
            bank_name,
            account_no,
            Some(amount_vnd),
            add_info.as_deref(),
            non_empty(account_name),
        )
        .ok_or_else(|| anyhow!("Không tạo được VietQR"))?;
        let prepared = self.prepare_chat_qr_image(order_no, &qr_url).await?;
        let nick = self.resolve_from_nickname(order_no, order).await;
        self.chat.send_image(
            order_no,
            &prepared.url,
            prepared.width,
            prepared.height,
            nick.as_deref(),
        )?;
        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
        let msgs = self.client.list_chat_messages(order_no).await?;
        if BotBinanceClient::chat_has_self_image(&msgs) {
            Ok(())
        } else {
            Err(anyhow!("Gửi lại QR nhưng chưa thấy trong chat"))
        }
    }

    /// Tin 3 — khi lệnh COMPLETED thật.
    async fn send_complete(&self, order_no: &str, order: &Value) -> Result<()> {
        if !self.chat.is_connected() {
            return Err(anyhow!("Chat WebSocket chưa kết nối, sẽ thử lại ở chu kỳ sau"));
        }
        if self.cfg.instruction_message.trim().is_empty() {
            return Ok(());
        }

        let amount_vnd = parse_amount_vnd(order).unwrap_or(0);
        let buyer = normalize_person_name(
            order
                .get("buyerName")
                .or_else(|| order.get("buyerRealName"))
                .and_then(|x| x.as_str())
                .unwrap_or(""),
        );
        let content = self
            .cfg
            .instruction_message
            .replace("{ma_lenh}", order_no)
            .replace("{so_tien}", &format_vnd(amount_vnd))
            .replace("{ten_nguoi_mua}", &buyer);
        if content.trim().is_empty() {
            return Ok(());
        }

        let nick = self.resolve_from_nickname(order_no, order).await;
        self.chat
            .send_text(order_no, &content, nick.as_deref())
            .map_err(|e| anyhow!("Gửi tin hoàn tất thất bại: {e}"))?;
        Ok(())
    }

    async fn resolve_from_nickname(&self, order_no: &str, order: &Value) -> Option<String> {
        if let Ok(msgs) = self.client.list_chat_messages(order_no).await {
            if let Some(n) = msgs.iter().find_map(|m| {
                if m.get("self").and_then(|s| s.as_bool()) == Some(true) {
                    m.get("fromNickName")
                        .and_then(|x| x.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            }) {
                return Some(n);
            }
        }
        order
            .get("sellerNickname")
            .or_else(|| order.get("sellerNickName"))
            .or_else(|| order.get("nickName"))
            .and_then(|x| x.as_str())
            .map(str::to_string)
    }

    async fn prepare_chat_qr_image(&self, order_no: &str, qr_url: &str) -> Result<PreparedChatImage> {
        let oid = app_log::oid(order_no);
        let qr_bytes = self
            .client
            .download_image(qr_url)
            .await
            .map_err(|e| anyhow!("Tải ảnh VietQR thất bại: {e}"))?;

        let (jpeg_bytes, width, height) =
            to_jpeg_bytes(&qr_bytes).map_err(|e| anyhow!("Chuyển QR sang JPEG thất bại: {e}"))?;

        let image_name = format!("{order_no}.jpg");
        let (upload_url, image_url) = self
            .client
            .presign_chat_image(&image_name, Some(order_no))
            .await
            .map_err(|e| anyhow!("presign: {e}"))?;

        self.client
            .upload_image_with_type(&upload_url, jpeg_bytes, "image/jpeg")
            .await
            .map_err(|e| anyhow!("upload: {e}"))?;

        // Chỉ dùng URL từ presign (+ public CDN cùng path). Không dùng host giả (static.binance.com).
        let candidates = preferred_image_urls(&image_url);
        // CDN đôi khi 403 ngay sau PUT — thử lại vài lần.
        let mut last_err = String::new();
        for attempt in 1..=5 {
            let wait_ms = 900u64 * attempt as u64;
            tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
            for candidate in &candidates {
                match self.client.verify_image_url(candidate).await {
                    Ok(()) => {
                        return Ok(PreparedChatImage {
                            url: candidate.clone(),
                            width,
                            height,
                        });
                    }
                    Err(e) => last_err = e.to_string(),
                }
            }
        }

        Err(anyhow!("CDN QR {oid} chưa sẵn: {last_err}"))
    }

    fn save_state(&self) {
        self.save_state_ref();
    }
}

async fn wait_until_stopped(stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

pub async fn run_session(
    cfg: BotConfig,
    api_key: String,
    api_secret: String,
    state_path: PathBuf,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    crate::bot::config::validate_config(&cfg)?;

    if !cfg.bank_name.trim().is_empty() {
        app_log::log(format!(
            "[INIT] TK dự phòng: {} {}",
            cfg.bank_name.trim(),
            cfg.account_no.trim()
        ));
    }

    let qr_note = if cfg.qr_transfer_content.trim().is_empty() {
        "(bank)"
    } else {
        cfg.qr_transfer_content.as_str()
    };
    app_log::log(format!("[INIT] CK QR: {qr_note}"));

    let client = Arc::new(BotBinanceClient::new(api_key, api_secret));

    client
        .sync_time()
        .await
        .map_err(|e| anyhow!("Không kết nối được Binance: {e}"))?;

    let recent = client
        .list_recent_sell_orders(60 * 60 * 1000)
        .await
        .map_err(|e| anyhow!("API key không hợp lệ hoặc thiếu quyền C2C: {e}"))?;
    let summary: Vec<String> = recent
        .iter()
        .filter_map(|o| {
            let no = o.get("orderNumber")?.as_str()?;
            let status = o
                .get("orderStatus")
                .or_else(|| o.get("status"))
                .map(|s| match s {
                    Value::String(x) => x.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| "?".into());
            Some(format!("{}={}", app_log::oid(no), status.trim_matches('"')))
        })
        .collect();
    app_log::log(format!(
        "[INIT] API OK · {} lệnh/1h{}",
        recent.len(),
        if summary.is_empty() {
            String::new()
        } else {
            format!(" · {}", summary.join(", "))
        }
    ));

    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }

    let chat = ChatManager::start(client.clone());
    let mut bot = Bot::new(cfg, client, chat, state_path);
    bot.run(stop).await
}

#[derive(Debug, Default)]
struct PaymentInfo {
    bank_name: String,
    account_no: String,
    account_name: String,
    amount_vnd: Option<i64>,
    buyer_name: String,
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Để trống / "(mac dinh)" / "default" → không nhúng addInfo (ngân hàng tự điền).
fn resolve_qr_add_info(template: &str, render: &dyn Fn(&str) -> String) -> Option<String> {
    let raw = template.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.eq_ignore_ascii_case("(mac dinh)")
        || raw.eq_ignore_ascii_case("(mặc định)")
        || raw.eq_ignore_ascii_case("default")
        || raw.eq_ignore_ascii_case("bank")
    {
        return None;
    }
    let rendered = render(raw);
    let t = rendered.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Binance đôi khi trả `Ð` (U+00D0 Eth) thay vì `Đ`/`D` → app ngân hàng bỏ mất chữ → "TRAN UC HIEU".
fn normalize_person_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|c| match c {
            'Ð' | 'Đ' => 'D',
            'ð' | 'đ' => 'd',
            _ => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_payment_info(detail: &Value) -> PaymentInfo {
    let mut info = PaymentInfo {
        amount_vnd: parse_amount_vnd(detail),
        buyer_name: detail
            .get("buyerName")
            .or_else(|| detail.get("buyerRealName"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        ..Default::default()
    };

    let fields = detail
        .get("payMethods")
        .or_else(|| detail.get("payMethod"))
        .and_then(|pm| if pm.is_array() { pm.get(0) } else { Some(pm) })
        .and_then(|pm| pm.get("fields"))
        .and_then(|f| f.as_array());

    let Some(fields) = fields else {
        return info;
    };

    for field in fields {
        let name = field
            .get("fieldName")
            .or_else(|| field.get("name"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_lowercase();
        let value = field
            .get("fieldValue")
            .or_else(|| field.get("value"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if value.is_empty() {
            continue;
        }
        if name.contains("bank")
            || name.contains("ngân hàng")
            || name.contains("ngan hang")
            || name == "identifier"
        {
            if info.bank_name.is_empty() {
                info.bank_name = value;
            }
        } else if name.contains("account")
            || name.contains("số tài khoản")
            || name.contains("so tai khoan")
            || name.contains("stk")
            || name.contains("card")
        {
            if info.account_no.is_empty() {
                info.account_no = value;
            }
        } else if (name.contains("name") || name.contains("tên") || name.contains("ten"))
            && (name.contains("họ và tên")
                || name.contains("ho va ten")
                || name.contains("họ tên")
                || name.contains("ho ten")
                || name.contains("chủ tài khoản")
                || name.contains("chu tai khoan")
                || name.contains("account name")
                || name.contains("full name")
                || name == "name"
                || name == "tên"
                || name == "ten")
        {
            info.account_name = value;
        }
    }
    info
}

fn order_status_code(order: &Value) -> Option<i64> {
    let status = order
        .get("orderStatus")
        .or_else(|| order.get("status"))
        .or_else(|| order.get("order_status"))?;
    if let Some(n) = status.as_i64() {
        return Some(n);
    }
    let s = status.as_str()?.trim();
    if let Ok(n) = s.parse::<i64>() {
        return Some(n);
    }
    match s.to_ascii_uppercase().as_str() {
        "PENDING" | "TRADING" => Some(1),
        "BUYER_PAYED" | "PAID" => Some(2),
        "DISTRIBUTING" | "VERIFYING" => Some(3),
        "COMPLETED" => Some(4),
        "IN_APPEAL" | "APPEALING" => Some(5),
        "CANCELLED" | "TIMEOUT" => Some(6),
        "CANCELLED_BY_SYSTEM" => Some(7),
        _ => None,
    }
}

fn is_pending_payment_code(code: Option<i64>) -> bool {
    code == Some(1)
}

fn is_buyer_paid_code(code: Option<i64>) -> bool {
    code == Some(2)
}

fn is_distributing_code(code: Option<i64>) -> bool {
    code == Some(3)
}

fn is_completed_code(code: Option<i64>) -> bool {
    code == Some(4)
}

fn get_i64(order: &Value, key: &str) -> i64 {
    order
        .get(key)
        .and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0)
}

fn parse_amount_vnd(order: &Value) -> Option<i64> {
    let raw = order.get("totalPrice")?;
    let num = raw
        .as_f64()
        .or_else(|| raw.as_str().and_then(|s| s.trim().replace(',', "").parse::<f64>().ok()))?;
    if num <= 0.0 {
        return None;
    }
    Some(num.round() as i64)
}

fn format_vnd(amount: i64) -> String {
    let s = amount.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

fn to_jpeg_bytes(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).map_err(|e| anyhow!("decode ảnh: {e}"))?;
    let width = img.width();
    let height = img.height();
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok((bytes.to_vec(), width, height));
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow!("encode jpeg: {e}"))?;
    let out = out.into_inner();
    if !out.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(anyhow!("encode jpeg không tạo được magic JPEG"));
    }
    Ok((out, width, height))
}

/// URL hợp lệ để gửi chat: đúng URL presign, hoặc cùng path trên public.bnbstatic.com.
fn preferred_image_urls(primary: &str) -> Vec<String> {
    let mut out = vec![primary.to_string()];
    if let Some(idx) = primary.find("/client_upload/") {
        let path = primary[idx..].split('?').next().unwrap_or("");
        if !path.is_empty() {
            for base in ["https://bin.bnbstatic.com", "https://public.bnbstatic.com"] {
                let u = format!("{base}{path}");
                if !out.iter().any(|x| x == &u) {
                    out.push(u);
                }
            }
        }
    }
    out
}
