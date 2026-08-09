use std::collections::HashMap;

/// Nhãn tiếng Việt cho mã trạng thái lệnh của Binance P2P.
///
/// Bản trước nạp nhãn từ `db/stage_map.json` cạnh file thực thi. File đó không nằm
/// trong `bundle.resources` của `tauri.conf.json`, nên bản build luôn dùng giá trị
/// mặc định — nhánh đọc file chỉ chạy được khi lập trình viên tự đặt file vào đúng
/// chỗ. Giữ nhãn ngay trong code thì hành vi giống nhau ở mọi máy.
#[derive(Debug, Clone)]
pub struct StageMap {
    labels: HashMap<i64, String>,
}

impl Default for StageMap {
    fn default() -> Self {
        let labels = [
            (1, "Đang chờ thanh toán"),
            (2, "Chờ người bán xác nhận"),
            (3, "Đang giải phóng coin"),
            (4, "Đã hoàn thành"),
            (5, "Đang khiếu nại"),
            (6, "Đã hủy"),
            (7, "Hủy bởi hệ thống"),
        ]
        .into_iter()
        .map(|(code, label)| (code, label.to_string()))
        .collect();

        Self { labels }
    }
}

impl StageMap {
    pub fn label(&self, code: i64) -> String {
        self.labels
            .get(&code)
            .cloned()
            .unwrap_or_else(|| format!("Không rõ ({code})"))
    }
}

#[cfg(test)]
mod tests {
    use super::StageMap;

    #[test]
    fn known_codes_have_labels() {
        let map = StageMap::default();
        assert_eq!(map.label(1), "Đang chờ thanh toán");
        assert_eq!(map.label(2), "Chờ người bán xác nhận");
        assert_eq!(map.label(4), "Đã hoàn thành");
    }

    #[test]
    fn unknown_code_is_reported_as_such() {
        assert_eq!(StageMap::default().label(-1), "Không rõ (-1)");
    }
}
