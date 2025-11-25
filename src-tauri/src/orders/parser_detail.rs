use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OrderDetailParsed {
    pub order_number: String,
    pub order_status_raw: u8,
    pub payment_fields: Vec<PaymentField>,
    pub remark: Option<String>,
    pub expected_pay_time_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PaymentField {
    pub field_type: String,
    pub field_value: Option<String>
}

pub fn parse_order_detail(root: &Value) -> Option<OrderDetailParsed> {
    let data = root.get("data")?.get("data")?;
    let order_number = data.get("orderNumber").and_then(|x| x.as_str())?.to_string();
    let order_status_raw = data.get("orderStatus").and_then(|x| x.as_i64()).unwrap_or(-1) as u8;
    let mut payment_fields = vec![];

    if let Some(pm_arr) = data.get("payMethods").and_then(|x| x.as_array()) {
        if let Some(first) = pm_arr.get(0) {
            if let Some(fields) = first.get("fields").and_then(|x| x.as_array()) {
                for f in fields {
                    payment_fields.push(PaymentField {
                        field_type: f.get("fieldContentType").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        field_value: f.get("fieldValue").and_then(|x| x.as_str()).map(|s| s.to_string()),
                    });
                }
            }
        }
    }
    let remark = data.get("remark").and_then(|x| x.as_str()).map(|s| s.to_string());
    let expected_pay_time_ms = data.get("expectedPayTime").and_then(|x| x.as_i64());

    Some(OrderDetailParsed {
        order_number,
        order_status_raw,
        payment_fields,
        remark,
        expected_pay_time_ms
    })
}