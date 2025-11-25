use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::Serialize;

use super::status::OrderStage;
use super::parser_list::{OrderSummaryParsed, PaymentField as ListField};
use super::parser_detail::{OrderDetailParsed, PaymentField as DetailField};

#[derive(Debug, Clone)]
pub struct Order {
    pub order_number: String,
    pub trade_type: String,
    pub asset: String,
    pub fiat: String,
    pub amount_asset: String,
    pub total_fiat: String,
    pub price: String,
    pub stage: OrderStage,
    #[allow(dead_code)]
    pub create_time_ms: i64, // Reserved for future use
    pub buyer_nick: String,
    pub seller_nick: String,
    pub account_name: Option<String>,
    pub account_no: Option<String>,
    pub bank_name: Option<String>,
    pub sub_bank: Option<String>,
    pub qr_code: Option<String>,
    pub remark: Option<String>,
    pub expected_pay_time_ms: Option<i64>,
    pub last_update_ts: i64
}

#[derive(Default)]
pub struct OrderStore {
    inner: RwLock<HashMap<String, Order>>,
    my_nickname: RwLock<Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct OrderView {
    pub order_number: String,
    pub side_role: String,
    pub trade_type: String,
    pub fiat: String,
    pub asset: String,
    pub amount_asset: String,
    pub total_fiat: String,
    pub price: String,
    pub stage_label: String,
    pub account_name: String,
    pub account_no: String,
    pub bank_name: String,
    pub last_update_ts: i64
}

impl OrderStore {
    pub async fn set_my_nickname(&self, nick: String) {
        *self.my_nickname.write().await = Some(nick);
    }

    pub async fn upsert_summaries(&self, list: Vec<OrderSummaryParsed>, ts: i64) {
        let mut g = self.inner.write().await;
        for s in list {
            let entry = g.entry(s.order_number.clone()).or_insert_with(|| Order {
                order_number: s.order_number.clone(),
                trade_type: s.trade_type.clone(),
                asset: s.asset.clone(),
                fiat: s.fiat.clone(),
                amount_asset: s.amount_asset.clone(),
                total_fiat: s.total_fiat.clone(),
                price: s.price.clone(),
                stage: OrderStage::from_code(s.order_status_raw),
                create_time_ms: s.create_time_ms,
                buyer_nick: s.buyer_nick.clone(),
                seller_nick: s.seller_nick.clone(),
                account_name: None, account_no: None, bank_name: None, sub_bank: None,
                qr_code: None, remark: None, expected_pay_time_ms: None,
                last_update_ts: ts,
            });

            entry.stage = OrderStage::from_code(s.order_status_raw);
            entry.amount_asset = s.amount_asset;
            entry.total_fiat = s.total_fiat;
            entry.price = s.price;
            entry.last_update_ts = ts;

            if let Some(fields) = s.payment_fields {
                self.apply_list_fields(entry, &fields);
            }
        }
    }

    pub async fn upsert_detail(&self, d: OrderDetailParsed, ts: i64) {
        let mut g = self.inner.write().await;
        let entry = g.entry(d.order_number.clone()).or_insert_with(|| Order {
            order_number: d.order_number.clone(),
            trade_type: "".into(),
            asset: "".into(),
            fiat: "".into(),
            amount_asset: "".into(),
            total_fiat: "".into(),
            price: "".into(),
            stage: OrderStage::from_code(d.order_status_raw),
            create_time_ms: 0,
            buyer_nick: "".into(),
            seller_nick: "".into(),
            account_name: None, account_no: None, bank_name: None, sub_bank: None,
            qr_code: None, remark: None, expected_pay_time_ms: None,
            last_update_ts: ts,
        });

        entry.stage = OrderStage::from_code(d.order_status_raw);
        entry.remark = d.remark;
        entry.expected_pay_time_ms = d.expected_pay_time_ms;
        entry.last_update_ts = ts;
        self.apply_detail_fields(entry, &d.payment_fields);
    }

    #[allow(dead_code)]
    pub async fn quick_update_status(&self, order_number: &str, code: u8, ts: i64) {
        // Reserved for future use - quick status updates without full re-parse
        let mut g = self.inner.write().await;
        if let Some(o) = g.get_mut(order_number) {
            o.stage = OrderStage::from_code(code);
            o.last_update_ts = ts;
        }
    }

    pub async fn list(&self) -> Vec<OrderView> {
        let g = self.inner.read().await;
        let me = self.my_nickname.read().await.clone();

        g.values().map(|o| {
            let side_role = if let Some(ref mine) = me {
                if o.buyer_nick == *mine { "YOU_BUY" }
                else if o.seller_nick == *mine { "YOU_SELL" }
                else { "OTHER" }
            } else { "UNKNOWN" };

            OrderView {
                order_number: o.order_number.clone(),
                side_role: side_role.into(),
                trade_type: o.trade_type.clone(),
                fiat: o.fiat.clone(),
                asset: o.asset.clone(),
                amount_asset: o.amount_asset.clone(),
                total_fiat: o.total_fiat.clone(),
                price: o.price.clone(),
                stage_label: o.stage.label(),
                account_name: o.account_name.clone().unwrap_or_default(),
                account_no: o.account_no.clone().unwrap_or_default(),
                bank_name: o.bank_name.clone().unwrap_or_default(),
                last_update_ts: o.last_update_ts
            }
        }).collect()
    }

    fn apply_list_fields(&self, entry: &mut Order, fields: &[ListField]) {
        for f in fields {
            match f.field_type.as_str() {
                "payee" => if let Some(v) = &f.field_value { entry.account_name = Some(v.clone()); },
                "pay_account" => if let Some(v) = &f.field_value { entry.account_no = Some(v.clone()); },
                "bank" => if let Some(v) = &f.field_value { entry.bank_name = Some(v.clone()); },
                "sub_bank" => if let Some(v) = &f.field_value { entry.sub_bank = Some(v.clone()); },
                "qr_code" => if let Some(v) = &f.field_value { entry.qr_code = Some(v.clone()); },
                _ => {}
            }
        }
    }

    fn apply_detail_fields(&self, entry: &mut Order, fields: &[DetailField]) {
        for f in fields {
            match f.field_type.as_str() {
                "payee" => if let Some(v) = &f.field_value { entry.account_name = Some(v.clone()); },
                "pay_account" => if let Some(v) = &f.field_value { entry.account_no = Some(v.clone()); },
                "bank" => if let Some(v) = &f.field_value { entry.bank_name = Some(v.clone()); },
                "sub_bank" => if let Some(v) = &f.field_value { entry.sub_bank = Some(v.clone()); },
                "qr_code" => if let Some(v) = &f.field_value { entry.qr_code = Some(v.clone()); },
                _ => {}
            }
        }
    }
}