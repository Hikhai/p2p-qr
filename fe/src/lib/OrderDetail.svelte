<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { toastSuccess, toastError } from './ToastContainer.svelte';
  
  export let order: any;
  export let onClose: () => void;
  
  let paymentDetail: any = null;
  let loadingPaymentDetail = true;
  let copiedField: string | null = null;

  function fmtDate(ms?: number) {
    if (!ms) return 'Không có';
    try { 
      return new Date(ms).toLocaleString('vi-VN', {
        year: 'numeric',
        month: '2-digit', 
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
      });
    } catch (e) { 
      return 'Không hợp lệ';
    }
  }

  function formatNumber(value: string | number, digits: number = 0) {
    if (!value) return '0';
    const num = typeof value === 'string' ? parseFloat(value) : value;
    if (isNaN(num)) return '0';
    return new Intl.NumberFormat('vi-VN', { 
      minimumFractionDigits: digits, 
      maximumFractionDigits: digits 
    }).format(num);
  }

  function formatCryptoAmount(value: string | number, asset: string) {
    if (!value) return '0';
    const num = typeof value === 'string' ? parseFloat(value) : value;
    if (isNaN(num)) return '0';
    
    // Tùy theo loại asset mà hiển thị số chữ số thập phân phù hợp
    let digits = 8; // Mặc định cho BTC và các altcoin
    
    if (asset === 'USDT' || asset === 'USDC' || asset === 'BUSD') {
      digits = 2; // Stablecoin chỉ hiển thị 2 chữ số thập phân
    } else if (asset === 'BTC') {
      digits = 6; // BTC hiển thị 6 chữ số
    } else if (asset === 'ETH') {
      digits = 4; // ETH hiển thị 4 chữ số
    }
    
    return new Intl.NumberFormat('vi-VN', { 
      minimumFractionDigits: 0,
      maximumFractionDigits: digits 
    }).format(num);
  }

  function getStatusColor(statusCode: number) {
    switch (statusCode) {
      case 1: return '#60a5fa'; // Đang chờ thanh toán
      case 2: return '#fbbf24'; // Đã thanh toán
      case 3: return '#f97316'; // Đang xác minh  
      case 4: return '#10b981'; // Đã hoàn thành
      case 5:
      case 6: return '#ef4444'; // Đã hủy
      default: return '#6b7280'; // Không xác định
    }
  }

  function getPaymentMethod() {
    // Tạm thời return default, sau này có thể expand
    if (order.bank_name) {
      return `Chuyển khoản ngân hàng - ${order.bank_name}`;
    }
    return 'Chuyển khoản ngân hàng';
  }

  $: sideRole = order.trade_type === 'BUY' ? 'Mua' : 'Bán';

  async function loadPaymentDetail() {
    console.log('[DEBUG] loadPaymentDetail called for order:', order.order_number);
    loadingPaymentDetail = true;
    paymentDetail = null;
    
    try {
      // First: Try to get from database (extension already captured it)
      const result = await invoke('get_order_payment_detail', { 
        orderNumber: order.order_number 
      });
      console.log('[DEBUG] Payment detail from DB:', result);
      
      if (result) {
        paymentDetail = result;
      } else {
        // Second: Try to get from browser localStorage (fallback)
        console.log('[DEBUG] Not in DB, checking localStorage...');
        await tryLoadFromLocalStorage();
      }
    } catch (error) {
      console.error('[DEBUG] Error loading payment details:', error);
    } finally {
      loadingPaymentDetail = false;
    }
  }
  
  async function tryLoadFromLocalStorage() {
    try {
      // Check if we have localStorage access (we're in the app's webview)
      const key = `p2p_payment_${order.order_number}`;
      const stored = localStorage.getItem(key);
      
      if (stored) {
        console.log('[DEBUG] Found payment detail in localStorage');
        const data = JSON.parse(stored);
        
        // Save to backend database
        try {
          await invoke('save_payment_detail_from_extension', {
            orderNumber: order.order_number,
            paymentDetail: {
              accountName: data.accountName,
              accountNo: data.accountNo,
              bankName: data.bankName,
              subBank: data.branchName || data.subBank,
              qrCodeUrl: data.qrCodeUrl || null,
              amount: data.amount,
              transferContent: data.transferContent,
              suggestedTransferContent: data.suggestedTransferContent || null
            }
          });
          console.log('[DEBUG] Saved localStorage data to backend');
          
          // Reload from DB to get the processed version (with generated QR)
          const result = await invoke('get_order_payment_detail', { 
            orderNumber: order.order_number 
          });
          paymentDetail = result;
        } catch (saveError) {
          console.error('[DEBUG] Failed to save to backend:', saveError);
          // Still show the data even if save failed
          paymentDetail = {
            account_name: data.accountName,
            account_no: data.accountNo,
            bank_name: data.bankName,
            sub_bank: data.branchName || data.subBank,
            qr_code_url: data.qrCodeUrl,
            amount: data.amount,
            transfer_content: data.transferContent,
            suggested_transfer_content: data.suggestedTransferContent
          };
        }
      } else {
        console.log('[DEBUG] No payment detail in localStorage either');
      }
    } catch (error) {
      console.error('[DEBUG] Error accessing localStorage:', error);
    }
  }

  // Load payment details when order changes (only for BUY orders)
  // Optimize: Track last loaded order to avoid re-loading same order
  let lastLoadedOrderNumber: string | null = null;
  $: if (order?.order_number && order?.trade_type === 'BUY' && order.order_number !== lastLoadedOrderNumber) {
    lastLoadedOrderNumber = order.order_number;
    loadPaymentDetail();
  }

  // Live-refresh when extension pushes updates
  let unlisten: UnlistenFn | null = null;
  onMount(async () => {
    try {
      unlisten = await listen('orders-updated', async (e: Event<any>) => {
        console.log('[DEBUG] Received orders-updated event:', e);
        const payload = e?.payload as any;
        console.log('[DEBUG] Event payload:', payload);
        // If event source is extension and applies to this order, reload payment detail
        if (!payload || payload.source !== 'extension') {
          console.log('[DEBUG] Ignoring event - not from extension');
          return;
        }
        if (!order?.order_number) {
          console.log('[DEBUG] No order number available');
          return;
        }
        if (!payload.orderNumber || String(payload.orderNumber) === String(order.order_number)) {
          console.log('[DEBUG] Reloading payment detail for order:', order.order_number);
          await loadPaymentDetail();
        }
      });
    } catch (err) {
      console.error('[DEBUG] Error setting up event listener:', err);
    }
  });
  onDestroy(() => { if (unlisten) { unlisten(); unlisten = null; } });
  
  // Copy to clipboard function
  async function copyToClipboard(text: string, fieldName: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedField = fieldName;
      toastSuccess(`Đã copy ${fieldName}`);
      
      // Reset copied state after 2 seconds
      setTimeout(() => {
        copiedField = null;
      }, 2000);
    } catch (error) {
      console.error('Failed to copy:', error);
      toastError('Không thể copy');
    }
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
              {order.status_label || `Code-${order.status_code}`}
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
            <span class="value amount">
              {formatNumber(order.total_fiat)} {order.fiat}
            </span>
          </div>
          
          <div class="detail-item">
            <span class="label">Giá:</span>
            <span class="value price">
              {formatNumber(order.price)} {order.fiat}
            </span>
          </div>
          
          <div class="detail-item">
            <span class="label">Số lượng {order.asset}:</span>
            <span class="value crypto-amount">
              {formatCryptoAmount(order.amount_asset, order.asset)} {order.asset}
            </span>
          </div>
          
          <div class="detail-item">
            <span class="label">Thời gian tạo:</span>
            <span class="value time">
              {fmtDate(order.create_time_ms)}
            </span>
          </div>
        </div>
      </div>

      {#if order.trade_type === 'BUY' && (order.status_code === 1 || order.status_code === 2 || order.status_code === 3)}
        <div class="detail-section">
          <h3>Thông tin thanh toán</h3>
        
        {#if loadingPaymentDetail}
            <div class="loading-state">
              <span>Đang tải thông tin thanh toán...</span>
            </div>
          {:else if paymentDetail}
            <div class="detail-grid">
              {#if paymentDetail.amount}
                <div class="detail-item">
                  <div class="detail-content">
                    <span class="label">Số tiền:</span>
                    <span class="value amount-highlight">{formatNumber(paymentDetail.amount)} VND</span>
                  </div>
                  <button 
                    class="copy-btn" 
                    class:copied={copiedField === 'số tiền'}
                    on:click={() => copyToClipboard(String(paymentDetail.amount), 'số tiền')}
                    title="Copy số tiền"
                  >
                    {#if copiedField === 'số tiền'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
                  </button>
                </div>
              {/if}
              
              {#if paymentDetail.transfer_content}
                <div class="detail-item highlight-item">
                  <div class="detail-content">
                    <span class="label">Nội dung chuyển khoản:</span>
                    <span class="value transfer-content">{paymentDetail.transfer_content}</span>
                  </div>
                  <button 
                    class="copy-btn primary" 
                    class:copied={copiedField === 'nội dung CK'}
                    on:click={() => copyToClipboard(paymentDetail.transfer_content, 'nội dung CK')}
                    title="Copy nội dung chuyển khoản"
                  >
                    {#if copiedField === 'nội dung CK'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
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
                    on:click={() => copyToClipboard(paymentDetail.account_name, 'tên chủ TK')}
                    title="Copy tên chủ tài khoản"
                  >
                    {#if copiedField === 'tên chủ TK'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
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
                    on:click={() => copyToClipboard(paymentDetail.bank_name, 'ngân hàng')}
                    title="Copy tên ngân hàng"
                  >
                    {#if copiedField === 'ngân hàng'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
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
                    on:click={() => copyToClipboard(paymentDetail.account_no, 'số TK')}
                    title="Copy số tài khoản"
                  >
                    {#if copiedField === 'số TK'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
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
                    on:click={() => copyToClipboard(paymentDetail.sub_bank, 'chi nhánh')}
                    title="Copy chi nhánh"
                  >
                    {#if copiedField === 'chi nhánh'}
                      <span in:scale>✓</span>
                    {:else}
                      📋
                    {/if}
                  </button>
                </div>
              {/if}
              
              {#if paymentDetail.suggested_transfer_content}
                <div class="detail-item">
                  <span class="label">Nội dung chuyển khoản đề xuất:</span>
                  <span class="value suggested-content">{paymentDetail.suggested_transfer_content}</span>
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
                  <span class="value">{fmtDate(paymentDetail.captured_at)}</span>
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
      <button 
        class="btn-primary"
        on:click={() => {
          const url = `https://c2c.binance.com/vi/fiatOrderDetail?orderNo=${order.order_number}&createdAt=${order.create_time}`;
          window.open(url, '_blank');
        }}
      >
        🌐 Mở trên Binance
      </button>
      <button class="btn-secondary" on:click={onClose}>Đóng</button>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
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
  
  .modal-content::-webkit-scrollbar {
    width: 8px;
  }
  
  .modal-content::-webkit-scrollbar-track {
    background: #111827;
    border-radius: 4px;
  }
  
  .modal-content::-webkit-scrollbar-thumb {
    background: #374151;
    border-radius: 4px;
  }
  
  .modal-content::-webkit-scrollbar-thumb:hover {
    background: #4b5563;
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
    padding: 0;
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: #f3f4f6;
  }

  .modal-body {
    padding: 24px;
  }

  .detail-section {
    margin-bottom: 24px;
  }

  .detail-section:last-child {
    margin-bottom: 0;
  }

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
    transition: all 0.3s ease;
    gap: 12px;
    overflow: hidden;
    box-sizing: border-box;
  }
  
  .detail-item:hover {
    background: rgba(17, 24, 39, 0.9);
    border-color: rgba(255, 255, 255, 0.08);
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
  
  .detail-item.highlight-item {
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.1) 0%, rgba(17, 24, 39, 0.6) 100%);
    border-color: rgba(37, 99, 235, 0.2);
  }
  
  .detail-item.highlight-item:hover {
    border-color: rgba(37, 99, 235, 0.4);
  }
  
  .detail-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .label {
    color: #9ca3af;
    font-size: 14px;
    font-weight: 500;
  }

  .value {
    color: #f3f4f6;
    font-size: 14px;
    font-weight: 600;
    text-align: right;
  }

  .value.status {
    font-weight: 700;
    padding: 4px 8px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
  }

  .trade-type.buy {
    color: #10b981;
  }

  .trade-type.sell {
    color: #ef4444;
  }

  .value.amount {
    color: #fbbf24;
  }

  .value.crypto-amount {
    color: #60a5fa;
  }

  .modal-footer {
    padding: 20px 24px;
    border-top: 1px solid #374151;
    display: flex;
    justify-content: flex-end;
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

  .btn-secondary:hover {
    background: #4b5563;
  }

  .loading-state {
    padding: 20px;
    text-align: center;
    color: #9ca3af;
    font-style: italic;
  }

  .no-payment-info {
    padding: 20px;
    text-align: center;
    color: #9ca3af;
    font-style: italic;
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
    max-width: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 12px;
    background: #ffffff;
    border-radius: 8px;
    overflow: hidden;
    box-sizing: border-box;
  }

  .qr-code-image {
    max-width: 100%;
    max-height: 200px;
    width: auto;
    height: auto;
    object-fit: contain;
    display: block;
  }

  .value.amount-highlight {
    color: #10b981;
    font-size: 16px;
    font-weight: 700;
  }

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

  /* Copy Button Styles */
  .copy-btn {
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.2);
    color: #60a5fa;
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.2s ease;
    flex-shrink: 0;
    min-width: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  
  .copy-btn:hover {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    transform: scale(1.05);
  }
  
  .copy-btn.primary {
    background: rgba(37, 99, 235, 0.15);
    border-color: rgba(37, 99, 235, 0.3);
  }
  
  .copy-btn.primary:hover {
    background: rgba(37, 99, 235, 0.25);
    border-color: rgba(37, 99, 235, 0.5);
  }
  
  .copy-btn.copied {
    background: rgba(16, 185, 129, 0.2);
    border-color: rgba(16, 185, 129, 0.4);
    color: #10b981;
  }
  
  .copy-btn span {
    display: inline-block;
  }
  
  /* Loading Animation */
  .loading-state {
    padding: 40px 20px;
    text-align: center;
    color: #9ca3af;
  }
  
  .loading-state::after {
    content: '...';
    animation: dots 1.5s steps(4, end) infinite;
  }
  
  @keyframes dots {
    0%, 20% { content: '.'; }
    40% { content: '..'; }
    60%, 100% { content: '...'; }
  }

  @media (max-width: 640px) {
    .modal-content {
      width: 95%;
      margin: 20px;
    }
    
    .detail-item {
      flex-direction: column;
      align-items: stretch;
    }
    
    .detail-content {
      flex-direction: column;
      align-items: flex-start;
      gap: 6px;
      margin-bottom: 8px;
    }
    
    .value {
      text-align: left;
    }
    
    .copy-btn {
      width: 100%;
    }

    .qr-code-image {
      max-width: 100%;
      max-height: 150px;
    }
  }
</style>