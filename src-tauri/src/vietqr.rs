//! Sinh URL ảnh QR chuyển khoản theo chuẩn VietQR.
//!
//! - Có cấu hình tên người chuyển → gửi `addInfo={tên} chuyen tien` vào QR.
//! - Không cấu hình → bỏ `addInfo` để app ngân hàng dùng nội dung mặc định.

use tracing::debug;

use crate::banks;

const TEMPLATE: &str = "compact2";

/// Sinh URL ảnh QR. Trả về `None` nếu không tra được ngân hàng hoặc số tài khoản
/// không hợp lệ.
///
/// `add_info`: nội dung chuyển khoản nhúng trong QR (chỉ khi user đã cấu hình).
/// Không được chứa mã lệnh.
pub fn image_url(
    bank_name: &str,
    account_no: &str,
    amount_vnd: Option<i64>,
    add_info: Option<&str>,
) -> Option<String> {
    let bin = banks::bank_bin(bank_name)?;
    let account = sanitize_account_no(account_no)?;

    let mut url = format!("https://img.vietqr.io/image/{bin}-{account}-{TEMPLATE}.jpg");

    let mut params: Vec<String> = Vec::new();
    if let Some(amount) = amount_vnd.filter(|a| *a > 0) {
        params.push(format!("amount={amount}"));
    }
    if let Some(info) = sanitize_add_info(add_info) {
        params.push(format!("addInfo={}", urlencoding_loose(&info)));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    debug!(bank = bank_name, bin, "đã sinh URL VietQR");
    Some(url)
}

/// Chuẩn hoá nội dung CK: bỏ mã lệnh / số thừa ở cuối, giới hạn độ dài.
pub fn sanitize_add_info(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_trailing_ref = strip_trailing_order_ref(trimmed);
    let cleaned = without_trailing_ref
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return None;
    }
    // VietQR addInfo thường giới hạn ngắn tuỳ ngân hàng; cắt an toàn.
    let bounded: String = cleaned.chars().take(50).collect();
    Some(bounded)
}

/// Bỏ các cụm số dài (≥5 chữ số) ở cuối chuỗi — thường là mã lệnh/ref Binance.
fn strip_trailing_order_ref(value: &str) -> &str {
    let mut end = value.len();
    let bytes = value.as_bytes();
    loop {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        let mut digit_start = end;
        while digit_start > 0 && bytes[digit_start - 1].is_ascii_digit() {
            digit_start -= 1;
        }
        let digit_len = end - digit_start;
        if digit_len >= 5 {
            end = digit_start;
            continue;
        }
        break;
    }
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &value[..end]
}

/// Encode tối thiểu cho query (giữ chữ/số, space → %20).
fn urlencoding_loose(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push_str("%20"),
            _ => {
                for b in c.to_string().as_bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

/// Số tài khoản đi thẳng vào đường dẫn URL nên phải kiểm tra trước khi dùng.
fn sanitize_account_no(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if !(4..=24).contains(&trimmed.chars().count()) {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Đọc số tiền VND từ chuỗi, chấp nhận cả cách viết Việt Nam và cách viết Anh Mỹ.
pub fn parse_vnd_amount(raw: &str) -> Option<i64> {
    let filtered: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();
    if filtered.is_empty() {
        return None;
    }

    let last_comma = filtered.rfind(',');
    let last_dot = filtered.rfind('.');

    let normalized = match (last_comma, last_dot) {
        (Some(comma), Some(dot)) => {
            let (decimal, thousands) = if comma > dot { (',', '.') } else { ('.', ',') };
            filtered.replace(thousands, "").replace(decimal, ".")
        }
        (Some(_), None) => normalize_single_separator(&filtered, ','),
        (None, Some(_)) => normalize_single_separator(&filtered, '.'),
        (None, None) => filtered,
    };

    let value: f64 = normalized.parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(value.round() as i64)
}

fn normalize_single_separator(value: &str, separator: char) -> String {
    let parts: Vec<&str> = value.split(separator).collect();
    let is_thousands = parts.len() > 2 || parts.last().is_some_and(|last| last.len() == 3);

    if is_thousands {
        value.replace(separator, "")
    } else {
        value.replace(separator, ".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vietnamese_grouping() {
        assert_eq!(parse_vnd_amount("27.000.000"), Some(27_000_000));
        assert_eq!(parse_vnd_amount("1.500"), Some(1_500));
        assert_eq!(parse_vnd_amount("27.000.000 đ"), Some(27_000_000));
    }

    #[test]
    fn parses_english_grouping() {
        assert_eq!(parse_vnd_amount("27,000,000"), Some(27_000_000));
        assert_eq!(parse_vnd_amount("1,500,000 VND"), Some(1_500_000));
    }

    #[test]
    fn decimal_part_is_not_treated_as_digits() {
        assert_eq!(parse_vnd_amount("1500000.00"), Some(1_500_000));
        assert_eq!(parse_vnd_amount("27000000.00"), Some(27_000_000));
        assert_eq!(parse_vnd_amount("27.000.000,50"), Some(27_000_001));
        assert_eq!(parse_vnd_amount("1,234.56"), Some(1_235));
    }

    #[test]
    fn small_amounts_are_not_multiplied() {
        assert_eq!(parse_vnd_amount("500"), Some(500));
        assert_eq!(parse_vnd_amount("999"), Some(999));
    }

    #[test]
    fn rejects_unparseable_or_non_positive() {
        assert_eq!(parse_vnd_amount(""), None);
        assert_eq!(parse_vnd_amount("abc"), None);
        assert_eq!(parse_vnd_amount("0"), None);
        assert_eq!(parse_vnd_amount("0.00"), None);
    }

    #[test]
    fn builds_url_without_transfer_content_when_unconfigured() {
        let url = image_url("Vietcombank", "1234567890", Some(27_000_000), None).unwrap();
        assert_eq!(
            url,
            "https://img.vietqr.io/image/970436-1234567890-compact2.jpg?amount=27000000"
        );
        assert!(!url.contains("addInfo"));
    }

    #[test]
    fn builds_url_with_configured_transfer_content() {
        let url = image_url(
            "Vietcombank",
            "1234567890",
            Some(200_000),
            Some("TRAN DUC HIEU chuyen tien"),
        )
        .unwrap();
        assert!(url.contains("amount=200000"));
        assert!(url.contains("addInfo=TRAN%20DUC%20HIEU%20chuyen%20tien"));
        assert!(!url.contains("173952"));
    }

    #[test]
    fn strips_trailing_order_number_from_add_info() {
        assert_eq!(
            sanitize_add_info(Some("TRAN DUC HIEU chuyen tien 173952")).as_deref(),
            Some("TRAN DUC HIEU chuyen tien")
        );
    }

    #[test]
    fn omits_amount_when_unknown() {
        let url = image_url("Vietcombank", "1234567890", None, None).unwrap();
        assert_eq!(
            url,
            "https://img.vietqr.io/image/970436-1234567890-compact2.jpg"
        );
    }

    #[test]
    fn rejects_unknown_bank_and_bad_account_number() {
        assert!(image_url("Ngân hàng Không Tồn Tại", "1234567890", None, None).is_none());
        assert!(image_url("Vietcombank", "12", None, None).is_none());
        assert!(image_url("Vietcombank", "../../etc/passwd", None, None).is_none());
        assert!(image_url("Vietcombank", "1234 5678", None, None).is_none());
    }
}
