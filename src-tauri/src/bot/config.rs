use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::banks;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotConfig {
    /// Tài khoản ngân hàng dự phòng — chỉ dùng khi lệnh thiếu payMethods.
    #[serde(default)]
    pub bank_name: String,
    #[serde(default)]
    pub account_no: String,
    #[serde(default)]
    pub account_name: String,

    /// Nội dung CK nhúng vào QR (`addInfo`).
    /// Để trống = dùng nội dung mặc định của ngân hàng (không ghi addInfo).
    /// Placeholder: {ma_lenh}, {so_tien}, {ten_nguoi_mua}.
    #[serde(default = "default_qr_content")]
    pub qr_transfer_content: String,

    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    #[serde(default = "default_max_age")]
    pub order_max_age_minutes: i64,

    #[serde(default = "default_greeting")]
    pub greeting_message: String,

    #[serde(default = "default_instruction")]
    pub instruction_message: String,
}

pub fn default_poll_interval() -> u64 {
    10
}
pub fn default_max_age() -> i64 {
    30
}
pub fn default_qr_content() -> String {
    // Mặc định: không nhúng addInfo → ngân hàng tự điền nội dung CK.
    String::new()
}

pub fn default_greeting() -> String {
    "TÍN ĐỨC KÍNH CHÀO QUÝ KHÁCH！\n\n\
LƯU Ý KHI CHUYỂN KHOẢN:\n\
1. Chuyển khoản nhanh 24/7 qua hệ thống Napas. Vui lòng ghi chính xác MÃ LỆNH trong nội dung chuyển khoản để giao dịch được mở tự động khi nhận đủ tiền.\n\
2. Không ghi các từ khóa liên quan đến tiền mã hoá như USDT, BUSD, BINANCE. Giao dịch có vi phạm sẽ bị huỷ.\n\
3. Cấm sử dụng tiền bẩn, tiền cờ bạc. Nếu phát hiện, Tín Đức sẽ giữ lệnh và liên hệ với Binance để xử lý.\n\
4. Vì hạn mức giao dịch ngân hàng, đội ngũ có thể sử dụng tài khoản của người thân để nhận tiền.\n\
5. Không giao dịch với nguồn tiền vi phạm pháp luật.\n\n\
Lưu ý: Nếu Quý khách không đồng ý với các quy định trên, vui lòng hủy giao dịch. Nếu đã chuyển tiền, chúng tôi sẽ xử lý theo quy định.\n\n\
UY TÍN – NHANH – CHÍNH XÁC！"
        .to_string()
}

pub fn default_instruction() -> String {
    "Để được mở khoá tự động, bạn vui lòng ghi ĐẦY ĐỦ mã lệnh {ma_lenh} vào nội dung chuyển khoản. \
Bạn cũng có thể quét mã QR được gửi cho bạn ở trên đã bao gồm số tiền và mã lệnh chính xác."
        .to_string()
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            bank_name: String::new(),
            account_no: String::new(),
            account_name: String::new(),
            qr_transfer_content: default_qr_content(),
            poll_interval_secs: default_poll_interval(),
            order_max_age_minutes: default_max_age(),
            greeting_message: default_greeting(),
            instruction_message: default_instruction(),
        }
    }
}

fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BinanceP2PManager")
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("bot_config.json")
}

pub fn state_path() -> PathBuf {
    app_data_dir().join("bot_state.json")
}

pub fn load_config() -> Result<BotConfig> {
    let path = config_path();
    if !path.exists() {
        let cfg = BotConfig::default();
        save_config(&cfg)?;
        return Ok(cfg);
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: BotConfig = serde_json::from_str(&raw)
        .map_err(|e| anyhow!("File {} không hợp lệ: {e}", path.display()))?;
    Ok(cfg)
}

pub fn save_config(cfg: &BotConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

pub fn validate_config(cfg: &BotConfig) -> Result<()> {
    if !cfg.bank_name.trim().is_empty() && banks::bank_bin(&cfg.bank_name).is_none() {
        return Err(anyhow!(
            "Không nhận diện được ngân hàng dự phòng '{}'. Dùng tên phổ biến như: MB, BIDV, Vietcombank, Techcombank, ACB...",
            cfg.bank_name
        ));
    }
    if cfg.poll_interval_secs < 5 {
        return Err(anyhow!("Chu kỳ quét tối thiểu 5 giây"));
    }
    if cfg.order_max_age_minutes < 5 {
        return Err(anyhow!("Tuổi lệnh tối thiểu 5 phút"));
    }
    Ok(())
}
