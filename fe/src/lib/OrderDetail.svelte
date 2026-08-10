<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { toastSuccess, toastError } from './ToastContainer.svelte';
  import { formatAsset, formatDateTime, formatFiat } from './format';
  import { type Order, type OrdersUpdatedPayload, type PaymentDetail } from './types';

  export let order: Order;
  export let onClose: () => void;

  let paymentDetail: PaymentDetail | null = null;
  let loadingPaymentDetail = false;
  let copiedField: string | null = null;
  let copyResetTimer: ReturnType<typeof setTimeout> | null = null;
  let lastLoadedOrderNumber: string | null = null;
  let unlisten: UnlistenFn | null = null;

  function getStatusColor(statusCode: number) {
    switch (statusCode) {
      case 1: return '#60a5fa';
      case 2: return '#fbbf24';
      case 3: return '#f97316';
      case 4: return '#10b981';
      case 5: return '#f97316';
      case 6:
      case 7: return '#ef4444';
      default: return '#6b7280';
    }
  }

  $: sideRole = order.trade_type === 'BUY' ? 'Mua' : 'Bán';
  // Chỉ hiện QR khi còn chờ thanh toán (status 1). Sau "Đã thanh toán" (2+) thì ẩn.
  $: showPayment = order.trade_type === 'BUY' && order.status_code === 1;

  async function loadPaymentDetail() {
    loadingPaymentDetail = true;
    try {
      paymentDetail = await invoke<PaymentDetail | null>('get_order_payment_detail', {
        orderNumber: order.order_number
      });
    } catch {
      paymentDetail = null;
    } finally {
      loadingPaymentDetail = false;
    }
  }

  $: if (showPayment && order.order_number !== lastLoadedOrderNumber) {
    lastLoadedOrderNumber = order.order_number;
    loadPaymentDetail();
  }

  onMount(async () => {
    try {
      unlisten = await listen<OrdersUpdatedPayload>('orders-updated', async (e) => {
        const payload = e.payload;
        if (payload?.source !== 'extension') return;
        if (!order?.order_number) return;
        if (!payload.orderNumber || String(payload.orderNumber) === String(order.order_number)) {
          await loadPaymentDetail();
        }
      });
    } catch {
      // Event listener optional when not running inside Tauri.
    }
  });

  onDestroy(() => {
    unlisten?.();
    if (copyResetTimer) clearTimeout(copyResetTimer);
  });

  async function copyToClipboard(text: string, fieldName: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedField = fieldName;
      toastSuccess(`Đã copy ${fieldName}`);
      if (copyResetTimer) clearTimeout(copyResetTimer);
      copyResetTimer = setTimeout(() => {
        copiedField = null;
      }, 2000);
    } catch {
      toastError('Không thể copy');
    }
  }

  function openOnBinance() {
    const url = `https://c2c.binance.com/vi/fiatOrderDetail?orderNo=${order.order_number}&createdAt=${order.create_time_ms}`;
    window.open(url, '_blank');
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="modal-overlay" on:click={onClose} transition:fade="{{ duration: 200 }}">
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="modal-content" on:click={(e) => e.stopPropagation()} transition:fly="{{ y: 50, duration: 300 }}">
    <div class="modal-header">
      <h2>Chi tiết lệnh #{order.order_number}</h2>
      <button class="close-btn" on:click={onClose}>×</button>
    </div>

    <div class="modal-body">
      <div class="detail-section">
        <h3>Thông tin cơ bản</h3>
        <div class="detail-grid">
          <div class="detail-item">
            <span class="label">Trạng thái lệnh:</span>
            <span class="value status" style="color: {getStatusColor(order.status_code)}">
              {order.status_label || `Không rõ (${order.status_code})`}
            </span>
          </div>

          <div class="detail-item">
            <span class="label">Loại lệnh:</span>
            <span class="value trade-type" class:buy={order.trade_type === 'BUY'} class:sell={order.trade_type === 'SELL'}>
              {sideRole} {order.asset}
            </span>
          </div>

          <div class="detail-item">
            <span class="label">Số tiền pháp định:</span>
            <span class="value amount">{formatFiat(order.total_fiat)} {order.fiat}</span>
          </div>

          <div class="detail-item">
            <span class="label">Giá:</span>
            <span class="value price">{formatFiat(order.price)} {order.fiat}</span>
          </div>

          <div class="detail-item">
            <span class="label">Số lượng {order.asset}:</span>
            <span class="value crypto-amount">{formatAsset(order.amount_asset, order.asset)} {order.asset}</span>
          </div>

          <div class="detail-item">
            <span class="label">Thời gian tạo:</span>
            <span class="value time">{formatDateTime(order.create_time_ms)}</span>
          </div>
        </div>
      </div>

      {#if showPayment}
        <div class="detail-section">
          <h3>Thông tin thanh toán</h3>

          {#if loadingPaymentDetail}
            <div class="loading-state"><span>Đang tải thông tin thanh toán</span></div>
          {:else if paymentDetail}
            <div class="detail-grid">
              {#if paymentDetail.amount}
                <div class="detail-item">
                  <div class="detail-content">
                    <span class="label">Số tiền:</span>
                    <span class="value amount-highlight">{formatFiat(paymentDetail.amount)} VND</span>
                  </div>
                  <button
                    class="copy-btn"
                    class:copied={copiedField === 'số tiền'}
                    on:click={() => copyToClipboard(String(paymentDetail?.amount), 'số tiền')}
                    title="Copy số tiền"
                  >
                    {#if copiedField === 'số tiền'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.transfer_content || paymentDetail.suggested_transfer_content}
                {@const ckContent = paymentDetail.transfer_content || paymentDetail.suggested_transfer_content || ''}
                <div class="detail-item highlight-item">
                  <div class="detail-content">
                    <span class="label">Nội dung chuyển khoản đề xuất:</span>
                    <span class="value transfer-content suggested-content">{ckContent}</span>
                  </div>
                  <button
                    class="copy-btn primary"
                    class:copied={copiedField === 'nội dung CK'}
                    on:click={() => copyToClipboard(ckContent, 'nội dung CK')}
                    title="Copy nội dung chuyển khoản"
                  >
                    {#if copiedField === 'nội dung CK'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.account_name}
                <div class="detail-item">
                  <div class="detail-content">
                    <span class="label">Họ và tên:</span>
                    <span class="value">{paymentDetail.account_name}</span>
                  </div>
                  <button
                    class="copy-btn"
                    class:copied={copiedField === 'tên chủ TK'}
                    on:click={() => copyToClipboard(paymentDetail?.account_name || '', 'tên chủ TK')}
                    title="Copy tên chủ tài khoản"
                  >
                    {#if copiedField === 'tên chủ TK'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.bank_name}
                <div class="detail-item">
                  <div class="detail-content">
                    <span class="label">Tên ngân hàng:</span>
                    <span class="value">{paymentDetail.bank_name}</span>
                  </div>
                  <button
                    class="copy-btn"
                    class:copied={copiedField === 'ngân hàng'}
                    on:click={() => copyToClipboard(paymentDetail?.bank_name || '', 'ngân hàng')}
                    title="Copy tên ngân hàng"
                  >
                    {#if copiedField === 'ngân hàng'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.account_no}
                <div class="detail-item highlight-item">
                  <div class="detail-content">
                    <span class="label">Số tài khoản/Số thẻ:</span>
                    <span class="value account-number">{paymentDetail.account_no}</span>
                  </div>
                  <button
                    class="copy-btn primary"
                    class:copied={copiedField === 'số TK'}
                    on:click={() => copyToClipboard(paymentDetail?.account_no || '', 'số TK')}
                    title="Copy số tài khoản"
                  >
                    {#if copiedField === 'số TK'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.sub_bank}
                <div class="detail-item">
                  <div class="detail-content">
                    <span class="label">Chi nhánh:</span>
                    <span class="value">{paymentDetail.sub_bank}</span>
                  </div>
                  <button
                    class="copy-btn"
                    class:copied={copiedField === 'chi nhánh'}
                    on:click={() => copyToClipboard(paymentDetail?.sub_bank || '', 'chi nhánh')}
                    title="Copy chi nhánh"
                  >
                    {#if copiedField === 'chi nhánh'}<span in:scale>✓</span>{:else}📋{/if}
                  </button>
                </div>
              {/if}

              {#if paymentDetail.qr_code_url}
                <div class="detail-item qr-code-section">
                  <span class="label">Mã QR:</span>
                  <div class="qr-code-container">
                    <img
                      src={paymentDetail.qr_code_url}
                      alt="QR Code thanh toán"
                      class="qr-code-image"
                    />
                  </div>
                </div>
              {/if}

              {#if paymentDetail.captured_at}
                <div class="detail-item">
                  <span class="label">Thời gian cập nhật:</span>
                  <span class="value">{formatDateTime(paymentDetail.captured_at)}</span>
                </div>
              {/if}
            </div>
          {:else}
            <div class="no-payment-info">
              <span>Chưa có thông tin thanh toán. Extension sẽ tự động cập nhật khi có dữ liệu từ network.</span>
            </div>
          {/if}
        </div>
      {/if}

      <div class="detail-section">
        <h3>Thông tin đối tác</h3>
        <div class="detail-grid">
          <div class="detail-item">
            <span class="label">{order.trade_type === 'BUY' ? 'Người bán' : 'Người mua'}:</span>
            <span class="value">
              {order.trade_type === 'BUY' ? order.seller_nickname : order.buyer_nickname}
            </span>
          </div>
        </div>
      </div>
    </div>

    <div class="modal-footer">
      <button class="btn-primary" on:click={openOnBinance}>🌐 Mở trên Binance</button>
      <button class="btn-secondary" on:click={onClose}>Đóng</button>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(4px);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .modal-content {
    background: linear-gradient(135deg, #1f2937 0%, #111827 100%);
    border-radius: 12px;
    width: 90%;
    max-width: 700px;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid #374151;
  }

  .modal-header h2 {
    margin: 0;
    color: #f3f4f6;
    font-size: 18px;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 24px;
    color: #9ca3af;
    cursor: pointer;
    width: 30px;
    height: 30px;
  }

  .close-btn:hover { color: #f3f4f6; }

  .modal-body { padding: 24px; }
  .detail-section { margin-bottom: 24px; }
  .detail-section:last-child { margin-bottom: 0; }
  .detail-section h3 {
    margin: 0 0 16px 0;
    color: #f3f4f6;
    font-size: 16px;
    font-weight: 600;
  }

  .detail-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 12px;
  }

  .detail-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 16px;
    background: rgba(17, 24, 39, 0.6);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.03);
    gap: 12px;
  }

  .detail-item.highlight-item {
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.1) 0%, rgba(17, 24, 39, 0.6) 100%);
    border-color: rgba(37, 99, 235, 0.2);
  }

  .detail-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .label { color: #9ca3af; font-size: 14px; font-weight: 500; }
  .value { color: #f3f4f6; font-size: 14px; font-weight: 600; text-align: right; }
  .value.status {
    font-weight: 700;
    padding: 4px 8px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
  }
  .trade-type.buy { color: #10b981; }
  .trade-type.sell { color: #ef4444; }
  .value.amount { color: #fbbf24; }
  .value.crypto-amount { color: #60a5fa; }
  .value.amount-highlight { color: #10b981; font-size: 16px; font-weight: 700; }
  .value.transfer-content {
    color: #fbbf24;
    font-family: monospace;
    font-size: 13px;
    background: #1f2937;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .value.suggested-content {
    color: #9ca3af;
    font-family: monospace;
    font-size: 13px;
    font-style: italic;
  }
  .value.account-number {
    color: #60a5fa;
    font-family: monospace;
    letter-spacing: 0.5px;
  }

  .modal-footer {
    padding: 20px 24px;
    border-top: 1px solid #374151;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
  }

  .btn-primary {
    background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
    color: white;
    border: none;
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
  }

  .btn-primary:hover {
    filter: brightness(1.05);
  }

  .btn-secondary {
    background: #374151;
    color: #f3f4f6;
    border: none;
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
  }

  .btn-secondary:hover { background: #4b5563; }

  .loading-state, .no-payment-info {
    padding: 20px;
    text-align: center;
    color: #9ca3af;
    font-style: italic;
  }

  .no-payment-info {
    background: #111827;
    border-radius: 6px;
    border-left: 4px solid #fbbf24;
  }

  .qr-code-section {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .qr-code-container {
    width: 100%;
    display: flex;
    justify-content: center;
    padding: 12px;
    background: #ffffff;
    border-radius: 8px;
  }

  .qr-code-image {
    max-width: 100%;
    max-height: 200px;
    object-fit: contain;
  }

  .copy-btn {
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.2);
    color: #60a5fa;
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    min-width: 40px;
  }

  .copy-btn.primary {
    background: rgba(37, 99, 235, 0.15);
    border-color: rgba(37, 99, 235, 0.3);
  }

  .copy-btn.copied {
    background: rgba(16, 185, 129, 0.2);
    border-color: rgba(16, 185, 129, 0.4);
    color: #10b981;
  }

  @media (max-width: 640px) {
    .modal-content { width: 95%; }
    .detail-item { flex-direction: column; align-items: stretch; }
    .detail-content {
      flex-direction: column;
      align-items: flex-start;
      gap: 6px;
      margin-bottom: 8px;
    }
    .value { text-align: left; }
    .copy-btn { width: 100%; }
  }
</style>
