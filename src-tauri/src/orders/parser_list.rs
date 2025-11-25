use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OrderSummaryParsed {
    pub order_number: String,
    pub trade_type: String,
    pub asset: String,
    pub fiat: String,
    pub amount_asset: String,
    pub total_fiat: String,
    pub price: String,
    pub order_status_raw: u8,
    pub create_time_ms: i64,
    pub buyer_nick: String,
    pub seller_nick: String,
    pub payment_fields: Option<Vec<PaymentField>>,
}

#[derive(Debug, Clone)]
pub struct PaymentField {
    pub field_type: String,
    pub field_value: Option<String>
}

/// Parse order-list responses
pub fn parse_order_list(root: &Value) -> Vec<OrderSummaryParsed> {
    let mut out = vec![];
    let arr_opt = root.get("data").and_then(|d| d.get("data").and_then(|x| x.as_array()).or_else(|| d.as_array()));
    let arr = match arr_opt { Some(a) => a, None => return out };

    for it in arr {
        let order_number = it.get("orderNumber").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if order_number.is_empty() { continue; }

        let trade_type = it.get("tradeType").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let asset = it.get("asset").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let fiat = it.get("fiat").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let amount_asset = it.get("amount").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let total_fiat = it.get("totalPrice").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let price = it.get("price").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let order_status_raw = it.get("orderStatus").and_then(|x| x.as_i64()).unwrap_or(-1) as u8;
        let create_time_ms = it.get("createTime").and_then(|x| x.as_i64()).unwrap_or(0);
        let buyer_nick = it.get("buyerNickname").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let seller_nick = it.get("sellerNickname").and_then(|x| x.as_str()).unwrap_or("").to_string();

        // Nếu list có payMethods.fields
        let payment_fields = it.get("payMethods")
            .and_then(|pm| pm.as_array())
            .and_then(|pmlist| {
                pmlist.get(0)
                    .and_then(|first| first.get("fields"))
                    .and_then(|f| f.as_array())
                    .map(|arrf| {
                        arrf.iter().map(|fitem| {
                            PaymentField {
                                field_type: fitem.get("fieldContentType").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                field_value: fitem.get("fieldValue").and_then(|x| x.as_str()).map(|s| s.to_string()),
                            }
                        }).collect()
                    })
            });

        out.push(OrderSummaryParsed {
            order_number,
            trade_type, asset, fiat,
            amount_asset, total_fiat, price,
            order_status_raw,
            create_time_ms,
            buyer_nick, seller_nick,
            payment_fields
        });
    }

    out
}