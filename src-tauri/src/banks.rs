//! Tra cứu mã BIN ngân hàng Việt Nam để sinh VietQR.
//!
//! Trước đây đây là một chuỗi `if name.contains(..) { return }` dài 190 dòng, nên
//! kết quả phụ thuộc vào thứ tự khai báo và có ba nhánh không bao giờ tới được:
//! `contains("sài gòn")` của SCB che mất SHB, `contains("shb")` che mất Shinhan,
//! `contains("scb")` che mất Standard Chartered.
//!
//! Cách so khớp ở đây không phụ thuộc thứ tự:
//!
//! 1. `codes` — mã viết tắt, chỉ khớp khi **trùng khít** cả chuỗi hoặc trùng một
//!    từ hoàn chỉnh. Cần vậy vì mã ngắn nằm lọt trong tên khác: "mb" nằm trong
//!    "sacombank", "vib" nằm trong "vietinbank".
//! 2. `aliases` — cụm từ đặc trưng, khớp theo `contains`, **alias dài nhất thắng**.
//!    Nhờ vậy "sai gon ha noi" (SHB) luôn thắng "sai gon" (SCB).

/// Một ngân hàng cùng các cách viết tên thường gặp.
pub struct Bank {
    pub bin: &'static str,
    /// Tên hiển thị chuẩn — giữ để đối chiếu/debug khi tra cứu.
    #[allow(dead_code)]
    pub name: &'static str,
    /// Mã viết tắt — chỉ khớp trùng khít hoặc trùng trọn một từ.
    pub codes: &'static [&'static str],
    /// Cụm từ đặc trưng — khớp theo `contains`, dài nhất thắng.
    /// Phải viết ở dạng đã chuẩn hoá: chữ thường, không dấu, cách nhau một khoảng trắng.
    pub aliases: &'static [&'static str],
}

/// Bảng BIN theo chuẩn NAPAS.
///
/// LƯU Ý: các mã dưới đây được giữ nguyên từ bản cài đặt trước, ngoại trừ BVBank
/// (xem bên dưới). Chúng **chưa được đối chiếu với bảng BIN chính thức của NAPAS**;
/// việc đó cần làm trước khi phát hành. Test `bins_are_unique` sẽ phát hiện nếu có
/// hai ngân hàng khác nhau dùng chung một mã.
pub static BANKS: &[Bank] = &[
    Bank {
        bin: "970407",
        name: "Techcombank",
        codes: &["tcb"],
        aliases: &["techcombank", "tech combank", "ky thuong"],
    },
    Bank {
        bin: "970403",
        name: "Sacombank",
        codes: &["stb"],
        aliases: &["sacombank", "sacom bank", "sai gon thuong tin"],
    },
    Bank {
        bin: "970436",
        name: "Vietcombank",
        codes: &["vcb"],
        aliases: &["vietcombank", "ngoai thuong"],
    },
    Bank {
        bin: "970422",
        name: "MB Bank",
        codes: &["mb", "mbb", "mbbank", "mbank"],
        aliases: &["mbbank", "mb bank", "quan doi"],
    },
    Bank {
        bin: "970415",
        name: "VietinBank",
        codes: &["ctg", "cti", "icb"],
        aliases: &["vietinbank", "vietin bank", "cong thuong viet nam"],
    },
    Bank {
        bin: "970418",
        name: "BIDV",
        codes: &["bid", "bidv"],
        aliases: &["bidv", "dau tu va phat trien"],
    },
    Bank {
        bin: "970405",
        name: "Agribank",
        codes: &["vba", "vbard"],
        aliases: &["agribank", "agri", "nong nghiep"],
    },
    Bank {
        bin: "970416",
        name: "ACB",
        codes: &["acb"],
        aliases: &["a chau", "asia commercial"],
    },
    Bank {
        bin: "970432",
        name: "VPBank",
        codes: &["vpb"],
        aliases: &["vpbank", "vp bank", "viet nam thinh vuong", "prosperity"],
    },
    Bank {
        bin: "970423",
        name: "TPBank",
        codes: &["tpb"],
        aliases: &["tpbank", "tp bank", "tienphong", "tien phong"],
    },
    Bank {
        bin: "970437",
        name: "HDBank",
        codes: &["hd", "hdb"],
        // Cố tình KHÔNG dùng "ho chi minh" / "tphcm" làm alias: chúng khớp cả tên
        // chi nhánh của ngân hàng khác (ví dụ "Vietcombank - CN Hồ Chí Minh").
        aliases: &["hdbank", "hd bank", "phat trien thanh pho ho chi minh"],
    },
    Bank {
        bin: "970441",
        name: "VIB",
        codes: &["vib"],
        aliases: &["quoc te", "international"],
    },
    Bank {
        bin: "970443",
        name: "SHB",
        codes: &["shb"],
        aliases: &["sai gon ha noi"],
    },
    Bank {
        bin: "970429",
        name: "SCB",
        codes: &["scb"],
        aliases: &["sai gon"],
    },
    Bank {
        bin: "970426",
        name: "MSB",
        codes: &["msb"],
        aliases: &["hang hai", "maritime"],
    },
    Bank {
        bin: "970448",
        name: "OCB",
        codes: &["ocb"],
        aliases: &["phuong dong", "orient"],
    },
    Bank {
        bin: "970440",
        name: "SeABank",
        codes: &["sea"],
        aliases: &["seabank", "sea bank", "dong nam a"],
    },
    Bank {
        bin: "970431",
        name: "Eximbank",
        codes: &["eib"],
        aliases: &["eximbank", "exim", "xuat nhap khau"],
    },
    Bank {
        bin: "970428",
        name: "NamABank",
        codes: &["nab"],
        aliases: &["namabank", "nam a bank", "nam a"],
    },
    Bank {
        bin: "970449",
        name: "LPBank",
        codes: &["lpb", "lvpb"],
        aliases: &["lpbank", "lienviet", "lien viet", "lvbank", "buu dien"],
    },
    Bank {
        bin: "970409",
        name: "BacABank",
        codes: &["bab"],
        aliases: &["bacabank", "bac a bank", "bac a"],
    },
    Bank {
        bin: "970427",
        name: "VietABank",
        codes: &["vab"],
        aliases: &["vietabank", "vieta bank", "viet a"],
    },
    Bank {
        bin: "970438",
        name: "BaoVietBank",
        codes: &["bvb"],
        aliases: &["baovietbank", "baoviet", "bao viet"],
    },
    Bank {
        bin: "970400",
        name: "SaigonBank",
        codes: &["sgb"],
        aliases: &["saigonbank", "saigon bank", "sai gon cong thuong"],
    },
    Bank {
        bin: "970430",
        name: "PGBank",
        codes: &["pgb"],
        aliases: &["pgbank", "pg bank", "xang dau", "petrolimex"],
    },
    Bank {
        bin: "970452",
        name: "KienLongBank",
        codes: &["klb"],
        aliases: &["kienlongbank", "kien long"],
    },
    Bank {
        bin: "970419",
        name: "NCB",
        codes: &["ncb"],
        aliases: &["quoc dan", "national citizen"],
    },
    Bank {
        bin: "970425",
        name: "ABBank",
        codes: &["abb"],
        aliases: &["abbank", "ab bank", "an binh"],
    },
    Bank {
        bin: "970412",
        name: "PVcomBank",
        codes: &["pvc"],
        aliases: &["pvcombank", "pvcom", "dai chung"],
    },
    Bank {
        bin: "970433",
        name: "VietBank",
        codes: &["vbb"],
        aliases: &["vietbank", "viet bank", "viet nam thuong tin"],
    },
    Bank {
        // Ngân hàng TMCP Bản Việt. Bản cũ để 970433 — trùng với VietBank, nên một
        // trong hai chắc chắn sai. 970454 là mã được ghi nhận cho BVBank.
        bin: "970454",
        name: "BVBank",
        codes: &["bvbank"],
        aliases: &["bvbank", "ban viet"],
    },
    Bank {
        bin: "970446",
        name: "Co-opBank",
        codes: &["coop"],
        aliases: &["coopbank", "co op bank", "cooperative", "hop tac xa"],
    },
    Bank {
        bin: "970414",
        name: "OceanBank",
        codes: &["ocean"],
        aliases: &["oceanbank", "ocean bank", "dai duong"],
    },
    Bank {
        bin: "970408",
        name: "GPBank",
        codes: &["gpb"],
        aliases: &["gpbank", "gp bank", "dau khi toan cau"],
    },
    Bank {
        bin: "970421",
        name: "VRB",
        codes: &["vrb"],
        aliases: &["viet nga", "lien doanh viet nga"],
    },
    Bank {
        bin: "970457",
        name: "Woori",
        codes: &[],
        aliases: &["woori"],
    },
    Bank {
        bin: "970424",
        name: "Shinhan Bank",
        codes: &["shbvn"],
        aliases: &["shinhan", "shbvn"],
    },
    Bank {
        bin: "458761",
        name: "HSBC",
        codes: &["hsbc"],
        aliases: &["hsbc", "hongkong", "hong kong"],
    },
    Bank {
        bin: "970410",
        name: "Standard Chartered",
        codes: &["scbvl"],
        aliases: &["standard chartered", "scbvl"],
    },
    Bank {
        bin: "970439",
        name: "PublicBank",
        codes: &["pbvn"],
        aliases: &["publicbank", "public bank"],
    },
    Bank {
        bin: "970434",
        name: "IndovinaBank",
        codes: &["ivb"],
        aliases: &["indovina"],
    },
    Bank {
        bin: "422589",
        name: "CIMB",
        codes: &["cimb"],
        aliases: &["cimb"],
    },
    Bank {
        bin: "970458",
        name: "UOB",
        codes: &["uob"],
        aliases: &["united overseas"],
    },
    Bank {
        bin: "546034",
        name: "Cake by VPBank",
        codes: &["cake"],
        aliases: &["cake"],
    },
    Bank {
        bin: "963369",
        name: "LioBank",
        codes: &["lio"],
        aliases: &["liobank", "lio bank"],
    },
    Bank {
        bin: "970461",
        name: "VikkiBank",
        codes: &["vikki"],
        aliases: &["vikkibank", "vikki bank", "ngan hang so vikki"],
    },
    Bank {
        bin: "999888",
        name: "VBSP",
        codes: &["vbsp"],
        aliases: &["chinh sach xa hoi", "chinh sach"],
    },
    Bank {
        bin: "970406",
        name: "VDB",
        codes: &["vdb"],
        aliases: &["phat trien viet nam"],
    },
];

/// Bỏ dấu tiếng Việt, chuyển về chữ thường.
fn fold_char(c: char) -> char {
    match c {
        'a' | 'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ' | 'ầ'
        | 'ẩ' | 'ẫ' | 'ậ' => 'a',
        'e' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
        'i' | 'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'o' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ' | 'ờ'
        | 'ở' | 'ỡ' | 'ợ' => 'o',
        'u' | 'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
        'y' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'd' | 'đ' => 'd',
        other => other,
    }
}

/// Chuẩn hoá tên ngân hàng về dạng so khớp được: chữ thường, không dấu, mọi ký tự
/// không phải chữ/số thành một khoảng trắng.
///
/// `"Ngân hàng TMCP Sài Gòn - Hà Nội (SHB)"` → `"ngan hang tmcp sai gon ha noi shb"`
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;

    for c in input.to_lowercase().chars() {
        let folded = fold_char(c);
        if folded.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(folded);
        } else {
            pending_space = true;
        }
    }
    out
}

/// Tra ngân hàng từ tên do Binance/extension gửi lên.
pub fn lookup(bank_name: &str) -> Option<&'static Bank> {
    let hay = normalize(bank_name);
    if hay.is_empty() {
        return None;
    }

    // 1. Trùng khít cả chuỗi với một mã viết tắt.
    if let Some(bank) = BANKS.iter().find(|b| b.codes.contains(&hay.as_str())) {
        return Some(bank);
    }

    // 2. Alias dài nhất thắng — loại bỏ hoàn toàn sự phụ thuộc thứ tự khai báo.
    let by_alias = BANKS
        .iter()
        .flat_map(|bank| {
            bank.aliases
                .iter()
                .filter(|alias| hay.contains(**alias))
                .map(move |alias| (alias.len(), bank))
        })
        .max_by_key(|(len, _)| *len);
    if let Some((_, bank)) = by_alias {
        return Some(bank);
    }

    // 3. Mã viết tắt xuất hiện như một từ hoàn chỉnh, ví dụ "ngan hang ... shb".
    let words: Vec<&str> = hay.split(' ').collect();
    BANKS
        .iter()
        .find(|b| b.codes.iter().any(|code| words.contains(code)))
}

/// Tra mã BIN từ tên ngân hàng.
pub fn bank_bin(bank_name: &str) -> Option<&'static str> {
    lookup(bank_name).map(|b| b.bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin_of(name: &str) -> &'static str {
        bank_bin(name).unwrap_or_else(|| panic!("không tra được BIN cho {name:?}"))
    }

    #[test]
    fn normalize_strips_diacritics_and_punctuation() {
        assert_eq!(
            normalize("Ngân hàng TMCP Sài Gòn - Hà Nội (SHB)"),
            "ngan hang tmcp sai gon ha noi shb"
        );
        assert_eq!(normalize("  MB  Bank "), "mb bank");
        assert_eq!(normalize("Đông Á"), "dong a");
    }

    /// Ba trường hợp mà chuỗi if/else cũ trả về sai ngân hàng.
    #[test]
    fn previously_unreachable_banks_now_resolve() {
        // "sai gon" của SCB từng chặn SHB
        assert_eq!(bin_of("Ngân hàng TMCP Sài Gòn - Hà Nội"), "970443");
        assert_eq!(bin_of("Ngân hàng TMCP Sài Gòn - Hà Nội (SHB)"), "970443");
        // "shb" từng chặn Shinhan
        assert_eq!(bin_of("shbvn"), "970424");
        // "scb" từng chặn Standard Chartered
        assert_eq!(bin_of("scbvl"), "970410");
    }

    /// Mã ngắn không được khớp khi chỉ là một đoạn nằm lọt trong tên khác.
    #[test]
    fn short_codes_do_not_match_substrings() {
        assert_eq!(bin_of("Sacombank"), "970403");
        assert_eq!(bin_of("Techcombank"), "970407");
        assert_eq!(bin_of("Vietcombank"), "970436");
        assert_eq!(bin_of("VietinBank"), "970415");
        assert_eq!(bin_of("Vikki"), "970461");
    }

    #[test]
    fn common_spellings_resolve() {
        for (name, bin) in [
            ("MB", "970422"),
            ("MB Bank", "970422"),
            ("MBBank", "970422"),
            ("Ngân hàng TMCP Quân Đội", "970422"),
            ("SCB", "970429"),
            ("Ngân hàng TMCP Sài Gòn", "970429"),
            ("SHB", "970443"),
            ("Vietcombank", "970436"),
            ("Ngân hàng Ngoại thương Việt Nam", "970436"),
            ("Techcombank", "970407"),
            ("Ngân hàng TMCP Kỹ Thương Việt Nam", "970407"),
            ("ACB", "970416"),
            ("Ngân hàng TMCP Á Châu", "970416"),
            ("BIDV", "970418"),
            ("Agribank", "970405"),
            ("VPBank", "970432"),
            ("TPBank", "970423"),
            ("Ngân hàng TMCP Tiên Phong", "970423"),
            ("HDBank", "970437"),
            ("VIB", "970441"),
            ("Ngân hàng TMCP Quốc tế Việt Nam", "970441"),
            ("MSB", "970426"),
            ("OCB", "970448"),
            ("SeABank", "970440"),
            ("Eximbank", "970431"),
            ("NamABank", "970428"),
            ("LPBank", "970449"),
            ("Sacombank", "970403"),
            ("Ngân hàng TMCP Sài Gòn Thương Tín", "970403"),
            ("SaigonBank", "970400"),
            ("Ngân hàng TMCP Sài Gòn Công Thương", "970400"),
            ("VietBank", "970433"),
            ("BVBank", "970454"),
            ("Ngân hàng TMCP Bản Việt", "970454"),
            ("Shinhan Bank", "970424"),
            ("Standard Chartered", "970410"),
            ("Woori", "970457"),
            ("Cake", "546034"),
        ] {
            assert_eq!(bin_of(name), bin, "sai BIN cho {name:?}");
        }
    }

    #[test]
    fn unknown_bank_returns_none() {
        assert_eq!(bank_bin("Ngân hàng Không Tồn Tại"), None);
        assert_eq!(bank_bin(""), None);
        assert_eq!(bank_bin("   "), None);
    }

    /// Hai ngân hàng khác nhau không được dùng chung một BIN — chính lỗi mà bản cũ
    /// mắc phải với BVBank và VietBank.
    #[test]
    fn bins_are_unique() {
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for b in BANKS {
            if let Some((other, _)) = seen.iter().find(|(_, bin)| *bin == b.bin) {
                panic!("BIN {} dùng cho cả {} và {}", b.bin, other, b.name);
            }
            seen.push((b.name, b.bin));
        }
    }

    /// Alias phải đã ở dạng chuẩn hoá, nếu không nó sẽ không bao giờ khớp.
    #[test]
    fn aliases_are_already_normalized() {
        for b in BANKS {
            for a in b.aliases {
                assert_eq!(normalize(a), **a, "alias {:?} của {} chưa chuẩn hoá", a, b.name);
            }
            for c in b.codes {
                assert_eq!(normalize(c), **c, "code {:?} của {} chưa chuẩn hoá", c, b.name);
            }
        }
    }
}
