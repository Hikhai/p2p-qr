/** Một lệnh P2P, khớp với `OrderRow` ở backend. */
export interface Order {
  order_number: string;
  trade_type: 'BUY' | 'SELL';
  fiat: string;
  asset: string;
  amount_asset: string;
  total_fiat: string;
  price: string;
  create_time_ms: number;
  status_code: number;
  status_label: string;
  buyer_nickname: string;
  seller_nickname: string;
  has_payment_detail: boolean;
  last_api_sync_ts: number;
}

/** Thông tin thanh toán, khớp với `PaymentDetail` ở backend. */
export interface PaymentDetail {
  account_name: string | null;
  account_no: string | null;
  bank_name: string | null;
  sub_bank: string | null;
  qr_code_url: string | null;
  amount: string | null;
  transfer_content: string | null;
  suggested_transfer_content: string | null;
  captured_at: number | null;
}

/** Thông tin credentials đã che, khớp với `CredentialInfo` ở backend. */
export interface CredentialInfo {
  label: string;
  api_key_masked: string;
  created_at: number;
  /** Tên chủ TK ngân hàng gắn Binance — dùng làm nội dung CK khi mua. */
  payer_bank_name?: string | null;
}

/** Payload của event `orders-updated`. */
export interface OrdersUpdatedPayload {
  source: 'incremental' | 'poll' | 'extension' | 'cleared';
  orderNumber?: string;
  changed?: number;
}

/** Các mã trạng thái được coi là đang xử lý. */
export const IN_PROGRESS_STATUS = [1, 2, 3];

export function isInProgress(order: Order): boolean {
  return IN_PROGRESS_STATUS.includes(order.status_code);
}
