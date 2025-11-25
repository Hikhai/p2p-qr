use serde::Deserialize;
use std::{collections::HashMap, fs};

#[derive(Debug, Clone, Deserialize)]
pub struct StageMapConfig { pub labels: HashMap<String, String> }

#[derive(Debug, Clone)]
pub struct StageMap { labels: HashMap<i64, String> }

impl StageMap {
    pub fn load_from(path: &str) -> Self {
        let mut labels: HashMap<i64, String> = HashMap::new();
        // defaults - map Binance P2P status codes (chính xác như trên sàn)
        labels.insert(1, "Đang chờ thanh toán".into());      // PENDING - chờ người mua thanh toán
        labels.insert(2, "Đã thanh toán".into());            // PAID - người mua đã thanh toán, chờ xác nhận
        labels.insert(3, "Đang xác minh".into());            // VERIFYING - đang xác minh thanh toán
        labels.insert(4, "Đã hoàn thành".into());            // COMPLETED - giao dịch thành công
        labels.insert(5, "Đã hủy".into());                   // CANCELLED - hủy bởi người dùng
        labels.insert(6, "Đã hủy bởi hệ thống".into());      // CANCELLED_BY_SYSTEM - hủy bởi hệ thống

        if let Ok(txt) = fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<StageMapConfig>(&txt) {
                for (k,v) in cfg.labels {
                    if let Ok(code) = k.parse::<i64>() { labels.insert(code, v); }
                }
            }
        }
        Self { labels }
    }

    pub fn label(&self, code: i64) -> String {
        self.labels.get(&code).cloned().unwrap_or_else(|| format!("Code{}", code))
    }
}
