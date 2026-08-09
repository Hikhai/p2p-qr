<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { fade, fly } from 'svelte/transition';
  import OrderTable from '../lib/OrderTable.svelte';
  import OrderDetail from '../lib/OrderDetail.svelte';
  import ToastContainer, { toastSuccess, toastError, toast } from '../lib/ToastContainer.svelte';
  import { formatDateTime, timeAgo } from '../lib/format';
  import { isInProgress, type CredentialInfo, type Order } from '../lib/types';

  let apiKey = "";
  let apiSecret = "";
  let label = "default";
  let orders: Order[] = [];
  let syncDays = 7;
  let activeTab:'dashboard'|'buy'|'sell'|'inprogress'|'settings' = 'dashboard';
  let loading = false;
  let errorMsg = "";
  let selectedOrder: Order | null = null;
  let refreshing = false;
  let lastRefreshTime = 0;
  let showApiKey = false;
  let showApiSecret = false;
  let isSaving = false;
  let isTesting = false;
  let syncProgress = "";
  let credentialInfo: CredentialInfo | null = null;
  let isClearing = false;
  let showClearConfirm = false;
  let payerBankName = "";

  $: credentialsSaved = credentialInfo !== null;

  async function loadOrders(silent: boolean = false) {
    try {
      orders = await invoke<Order[]>('list_orders_from_db', { limit: 0 });
      if (selectedOrder) {
        selectedOrder =
          orders.find((o) => o.order_number === selectedOrder!.order_number) ?? selectedOrder;
      }
      errorMsg = "";
    }
    catch (e) {
      errorMsg = String(e);
      if (!silent) {
        toastError('Không thể tải danh sách lệnh');
      }
    }
  }
  $: buyOrders = orders.filter(o => o.trade_type === 'BUY');
  $: sellOrders = orders.filter(o => o.trade_type === 'SELL');
  $: inProgressOrders = orders.filter(isInProgress);
  $: lastSync = orders.reduce((max, o) => Math.max(max, o.last_api_sync_ts || 0), 0);
  async function saveCreds() {
    errorMsg="";
    
    // Validation
    if (!apiKey.trim() || !apiSecret.trim()) {
      errorMsg = "API Key và API Secret không được để trống";
      toastError('Vui lòng nhập đầy đủ thông tin');
      return;
    }
    
    if (apiKey.length < 20 || apiSecret.length < 20) {
      errorMsg = "API Key và API Secret không hợp lệ (quá ngắn)";
      toastError('API Key/Secret không hợp lệ');
      return;
    }
    
    isSaving = true;
    const isUpdate = credentialsSaved;

    try {
      const result = await invoke<{ accountSwitched: boolean }>('store_api_credentials', {
        label,
        apiKey,
        apiSecret,
        payerBankName: payerBankName.trim() || null
      });
      errorMsg = "";
      // Xoá khỏi biến của trang ngay sau khi lưu: secret không cần nằm trong webview.
      apiKey = "";
      apiSecret = "";
      await loadCredentialInfo();
      if (result?.accountSwitched) {
        orders = [];
        selectedOrder = null;
        toastSuccess('Đã đổi tài khoản Binance — dữ liệu cũ đã xoá. Đang đồng bộ lại...');
        await doForceSync();
      } else {
        toastSuccess(isUpdate ? 'Đã cập nhật API credentials' : 'Đã lưu API credentials vào kho khoá hệ thống');
      }
    }
    catch (e) {
      errorMsg = String(e);
      toastError('Lưu thất bại: ' + String(e));
    }
    finally {
      isSaving = false;
    }
  }

  async function savePayerBankName() {
    if (!credentialsSaved) {
      toastError('Hãy lưu API credentials trước');
      return;
    }
    if (!payerBankName.trim()) {
      toastError('Nhập tên chủ tài khoản ngân hàng (người chuyển)');
      return;
    }
    try {
      await invoke('update_payer_bank_name', { payerBankName: payerBankName.trim() });
      await loadCredentialInfo();
      toastSuccess('Đã lưu tên chủ TK người chuyển');
    } catch (e) {
      toastError('Lưu tên chủ TK thất bại: ' + String(e));
    }
  }
  async function testCreds() {
    errorMsg="";

    if (!credentialsSaved) {
      errorMsg = "Vui lòng lưu API credentials trước khi test";
      toastError('Chưa lưu credentials');
      return;
    }

    isTesting = true;
    try {
      const message = await invoke<string>('test_api_credentials');
      errorMsg = "";
      toastSuccess(message);
    }
    catch (e) {
      errorMsg = "Kết nối thất bại: " + String(e);
      toastError('Kết nối thất bại — kiểm tra lại API Key/Secret');
    }
    finally {
      isTesting = false;
    }
  }
  async function removeCreds() {
    try {
      await invoke('clear_api_credentials');
      credentialInfo = null;
      toastSuccess('Đã xoá API credentials khỏi kho khoá hệ thống');
    } catch (e) {
      errorMsg = String(e);
      toastError('Xoá credentials thất bại');
    }
  }
  async function doForceSync() {
    if (!credentialsSaved) {
      errorMsg = "Vui lòng lưu API credentials trước khi đồng bộ";
      toastError('Chưa lưu credentials');
      return;
    }
    
    if (syncDays < 1 || syncDays > 30) {
      errorMsg = "Số ngày phải từ 1-30";
      toastError('Số ngày không hợp lệ');
      return;
    }
    
    loading = true; 
    errorMsg = "";
    syncProgress = "Đang kết nối với sàn...";
    toast('🔄 Bắt đầu đồng bộ dữ liệu...');
    
    try {
      syncProgress = `Đang tải dữ liệu ${syncDays} ngày gần nhất...`;
      const changed = await invoke<number>('force_initial_sync', { days: syncDays });

      syncProgress = "Đang xử lý và lưu dữ liệu...";
      await loadOrders();

      syncProgress = "";
      toastSuccess(`Đã đồng bộ ${orders.length} lệnh (${changed} lệnh mới hoặc có thay đổi)`);
      errorMsg = "";
    }
    catch (e) {
      errorMsg = "Đồng bộ thất bại: " + String(e);
      syncProgress = "";
      toastError('Đồng bộ thất bại — kiểm tra kết nối và credentials');
    }
    finally { 
      loading = false; 
      syncProgress = "";
    }
  }

  async function clearAllData() {
    if (!showClearConfirm) {
      showClearConfirm = true;
      return;
    }

    isClearing = true;
    errorMsg = "";

    try {
      await invoke('clear_all_data');
      // Không còn cần tắt app: dữ liệu được xoá bằng SQL ngay trong tiến trình.
      orders = [];
      credentialInfo = null;
      selectedOrder = null;
      showClearConfirm = false;
      toastSuccess('Đã xoá toàn bộ dữ liệu');
    } catch (e) {
      errorMsg = `Lỗi khi xóa dữ liệu: ${String(e)}`;
      toastError(errorMsg);
    } finally {
      isClearing = false;
    }
  }

  function cancelClear() {
    showClearConfirm = false;
  }



  function handleOrderClick(order: Order) {
    selectedOrder = order;
  }

  function closeOrderDetail() {
    selectedOrder = null;
  }

  async function refreshFromExchange() {
    refreshing = true;
    errorMsg = "";
    try {
      await invoke('force_sync_recent');
      await loadOrders(true);
      lastRefreshTime = Date.now();
      toastSuccess('Đã cập nhật từ sàn');
    } catch (e) {
      errorMsg = String(e);
      toastError('Cập nhật thất bại');
    } finally {
      refreshing = false;
    }
  }

  async function loadCredentialInfo() {
    try {
      credentialInfo = await invoke<CredentialInfo | null>('get_credential_info');
      if (credentialInfo?.payer_bank_name) {
        payerBankName = credentialInfo.payer_bank_name;
      }
    } catch (e) {
      errorMsg = String(e);
      credentialInfo = null;
    }
  }

  onMount(() => {
    loadOrders(true);
    loadCredentialInfo();

    // Backend chỉ bắn event khi dữ liệu thực sự đổi, nên đây là toàn bộ cơ chế tự
    // động cập nhật. Bản trước còn thêm setInterval 30 giây gọi lại force_sync_recent,
    // chạy song song với scheduler ở backend và nhân đôi số lời gọi API.
    let unlistenFn: UnlistenFn | undefined;
    listen('orders-updated', () => {
      lastRefreshTime = Date.now();
      loadOrders(true);
    })
      .then((unlisten) => { unlistenFn = unlisten; })
      .catch(() => {});

    return () => { unlistenFn?.(); };
  });
</script>

<style>
:global(html) { 
  background: linear-gradient(135deg, #0f172a 0%, #1e1b4b 100%);
  color: #e5e7eb; 
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  min-height: 100vh;
}

:global(body) {
  margin: 0;
  padding: 20px;
}

nav { 
  background: rgba(31, 41, 55, 0.8);
  backdrop-filter: blur(10px);
  padding: 16px 20px;
  border-radius: 12px;
  margin-bottom: 24px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

nav button { 
  background: rgba(37, 99, 235, 0.1);
  color: #60a5fa;
  border: 1px solid rgba(37, 99, 235, 0.3);
  padding: 10px 18px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  transition: all 0.2s ease;
}

nav button:hover:not(:disabled) { 
  background: rgba(37, 99, 235, 0.2);
  border-color: rgba(37, 99, 235, 0.5);
  transform: translateY(-2px);
}

nav button:disabled { 
  opacity: 1;
  background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
  color: white;
  border-color: #2563eb;
  cursor: default;
  transform: none;
}

.error { 
  color: #fca5a5; 
  font-size: 13px; 
  margin-top: 8px;
  padding: 12px;
  background: rgba(239, 68, 68, 0.1);
  border-left: 4px solid #ef4444;
  border-radius: 6px;
}

input { 
  background: #1f2937;
  color: #e5e7eb;
  border: 1px solid #374151;
  padding: 10px 14px;
  margin: 6px 0;
  border-radius: 6px;
  font-size: 13px;
  transition: all 0.2s ease;
  box-sizing: border-box;
}

input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
}

button { 
  background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
  color: white;
  border: none;
  padding: 10px 18px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  transition: all 0.2s ease;
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.3);
}

button:disabled { 
  opacity: 0.5;
  cursor: not-allowed;
  box-shadow: none;
}

button:hover:not(:disabled) { 
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(37, 99, 235, 0.4);
}

button:active:not(:disabled) {
  transform: translateY(0);
}

h2, h3 {
  color: #f3f4f6;
  font-weight: 700;
}

label {
  display: block;
  margin-bottom: 8px;
  color: #9ca3af;
  font-size: 13px;
  font-weight: 500;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin: 20px 0 32px 0;
}

.stat-card {
  background: linear-gradient(135deg, rgba(31, 41, 55, 0.8) 0%, rgba(17, 24, 39, 0.6) 100%);
  backdrop-filter: blur(10px);
  padding: 24px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  gap: 16px;
  transition: all 0.3s ease;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}

.stat-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  border-color: rgba(255, 255, 255, 0.1);
}

.stat-card.buy-card {
  border-left: 4px solid #10b981;
}

.stat-card.sell-card {
  border-left: 4px solid #ef4444;
}

.stat-card.progress-card {
  border-left: 4px solid #f59e0b;
}

.stat-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
}

.dot {
  display: inline-block;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  flex-shrink: 0;
}
.dot.total { background: #60a5fa; box-shadow: 0 0 0 3px rgba(96, 165, 250, 0.25); }
.dot.buy { background: #22c55e; box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.25); }
.dot.sell { background: #ef4444; box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.25); }
.dot.progress { background: #f59e0b; box-shadow: 0 0 0 3px rgba(245, 158, 11, 0.25); }

.stat-content {
  flex: 1;
}

.stat-label {
  font-size: 12px;
  color: #9ca3af;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-weight: 600;
  margin-bottom: 6px;
}

.stat-value {
  font-size: 28px;
  font-weight: 700;
  color: #f3f4f6;
}

.stat-value.buy {
  color: #10b981;
}

.stat-value.sell {
  color: #ef4444;
}

.stat-value.progress {
  color: #f59e0b;
}

.action-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 16px 0;
  flex-wrap: wrap;
}

.btn-action {
  display: flex;
  align-items: center;
  gap: 6px;
}

.auto-indicator {
  font-size: 12px;
  color: #4ade80;
  padding: 8px 12px;
  background: rgba(16, 185, 129, 0.1);
  border: 1px solid rgba(16, 185, 129, 0.25);
  border-radius: 6px;
}

.last-update {
  opacity: 0.7;
  font-size: 12px;
  color: #9ca3af;
  padding: 8px 12px;
  background: rgba(31, 41, 55, 0.5);
  border-radius: 6px;
}

.spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

a {
  color: #60a5fa;
  text-decoration: none;
  transition: color 0.2s ease;
}

a:hover {
  color: #93c5fd;
  text-decoration: underline;
}

h4 {
  color: #f3f4f6;
  font-weight: 600;
}

ul li {
  margin-bottom: 4px;
}

@media (max-width: 768px) {
  .stats-grid {
    grid-template-columns: 1fr 1fr;
  }
  
  .stat-card {
    padding: 16px;
  }
  
  .stat-icon {
    font-size: 24px;
  }
  
  .stat-value {
    font-size: 22px;
  }
  
  .action-bar {
    flex-direction: column;
    align-items: stretch;
  }
  
  .btn-action {
    justify-content: center;
  }
}
</style>

<nav>
  <button on:click={()=>activeTab='dashboard'} disabled={activeTab==='dashboard'}>Dashboard</button>
  <button on:click={()=>activeTab='buy'} disabled={activeTab==='buy'}>Lệnh mua</button>
  <button on:click={()=>activeTab='sell'} disabled={activeTab==='sell'}>Lệnh bán</button>
  <button on:click={()=>activeTab='inprogress'} disabled={activeTab==='inprogress'}>Đang xử lý</button>
  <button on:click={()=>activeTab='settings'} disabled={activeTab==='settings'}>Cài đặt</button>
  <span style="margin-left:12px;opacity:.8;font-size:12px;">Đồng bộ cuối: {formatDateTime(lastSync)}</span>
</nav>

{#if activeTab==='dashboard'}
  <div transition:fade>
    <h2>Tổng quan</h2>
    <div class="stats-grid">
      <div class="stat-card" transition:fly="{{ x: -20, delay: 0, duration: 300 }}">
        <div class="stat-icon"><span class="dot total"></span></div>
        <div class="stat-content">
          <div class="stat-label">Tất cả</div>
          <div class="stat-value">{orders.length}</div>
        </div>
      </div>
      
      <div class="stat-card buy-card" transition:fly="{{ x: -20, delay: 100, duration: 300 }}">
        <div class="stat-icon"><span class="dot buy"></span></div>
        <div class="stat-content">
          <div class="stat-label">Mua</div>
          <div class="stat-value buy">{buyOrders.length}</div>
        </div>
      </div>
      
      <div class="stat-card sell-card" transition:fly="{{ x: -20, delay: 200, duration: 300 }}">
        <div class="stat-icon"><span class="dot sell"></span></div>
        <div class="stat-content">
          <div class="stat-label">Bán</div>
          <div class="stat-value sell">{sellOrders.length}</div>
        </div>
      </div>
      
      <div class="stat-card progress-card" transition:fly="{{ x: -20, delay: 300, duration: 300 }}">
        <div class="stat-icon"><span class="dot progress"></span></div>
        <div class="stat-content">
          <div class="stat-label">Đang xử lý</div>
          <div class="stat-value progress">{inProgressOrders.length}</div>
        </div>
      </div>
    </div>
  </div>
  <div class="action-bar">
    <button class="btn-action" on:click={refreshFromExchange} disabled={refreshing}>
      {#if refreshing}
        <span class="spinner"></span> Đang cập nhật...
      {:else}
        🔄 Tải lại
      {/if}
    </button>
    <button class="btn-action" on:click={refreshFromExchange} disabled={refreshing}>
      {#if refreshing}
        <span class="spinner"></span> Đang cập nhật...
      {:else}
        📡 Cập nhật từ sàn
      {/if}
    </button>
    <span class="auto-indicator">🔄 Tự động theo dõi (15s)</span>
    <span class="last-update">
      Cập nhật cuối: {lastRefreshTime ? timeAgo(lastRefreshTime) : (lastSync ? timeAgo(lastSync) : '—')}
    </span>
  </div>
  {#if errorMsg}<div class="error">{errorMsg}</div>{/if}
  
  {#if orders.length === 0}
    <p style="color:#fbbf24; margin-top:10px;">Chưa có dữ liệu. Vào tab "Cài đặt" để cấu hình API và sync dữ liệu.</p>
  {:else}
    <div style="margin-top:16px;">
      <h3>Lệnh mới nhất</h3>
      <p style="color:#9ca3af; font-size:12px; margin-bottom:8px;">
        💡 Click vào lệnh để xem chi tiết (trừ lệnh đang chờ thanh toán)
      </p>
      <OrderTable list={orders.slice(0, 10)} onOrderClick={handleOrderClick} />
    </div>
  {/if}
{/if}

{#if activeTab==='buy'}
  <h2>Lệnh mua</h2>
  <OrderTable list={buyOrders} onOrderClick={handleOrderClick} />
{/if}

{#if activeTab==='sell'}
  <h2>Lệnh bán</h2>
  <OrderTable list={sellOrders} onOrderClick={handleOrderClick} />
{/if}

{#if activeTab==='inprogress'}
  <h2>Lệnh đang xử lý</h2>
  <OrderTable list={inProgressOrders} onOrderClick={handleOrderClick} />
{/if}

{#if activeTab==='settings'}
  <div transition:fade>
    <div style="max-width: 700px;">
      <h2>⚙️ Cài đặt API Credentials</h2>
      <p style="color:#9ca3af; font-size:13px; margin-bottom:20px;">
        Cấu hình API Key và Secret từ Binance P2P để đồng bộ dữ liệu lệnh giao dịch
      </p>
      
      <div style="background: rgba(31, 41, 55, 0.8); padding: 24px; border-radius: 12px; border: 1px solid rgba(255, 255, 255, 0.05); margin-bottom: 24px;">
        {#if credentialInfo}
          <div style="margin-bottom:20px; padding:12px; background:rgba(34, 197, 94, 0.1); border-left:4px solid #22c55e; border-radius:6px;">
            <p style="color:#4ade80; font-size:13px; margin:0 0 4px 0;">
              Đã lưu trong kho khoá của hệ điều hành ({credentialInfo.label})
            </p>
            <p style="color:#9ca3af; font-size:12px; margin:0; font-family:monospace;">
              API Key: {credentialInfo.api_key_masked} · lưu ngày {formatDateTime(credentialInfo.created_at)}
            </p>
          </div>
        {/if}

        <label>
          Label (Tên cấu hình):
          <input 
            bind:value={label} 
            placeholder="default" 
            style="width:100%; margin-top:8px; box-sizing:border-box;"
          />
        </label>
        
        <label style="margin-top:16px;">
          <div style="display:flex; justify-content:space-between; align-items:center;">
            <span>API Key:</span>
            <button 
              type="button"
              on:click={() => showApiKey = !showApiKey} 
              style="padding:4px 12px; font-size:11px; background:rgba(107, 114, 128, 0.3);"
            >
              {showApiKey ? '🙈 Ẩn' : '👁️ Hiện'}
            </button>
          </div>
          <input 
            bind:value={apiKey} 
            type={showApiKey ? 'text' : 'password'}
            placeholder={credentialInfo ? 'Nhập API Key mới để thay thế' : 'Nhập API Key từ Binance'}
            style="width:100%; margin-top:8px; font-family: monospace; box-sizing:border-box;"
          />
        </label>
        
        <label style="margin-top:16px;">
          <div style="display:flex; justify-content:space-between; align-items:center;">
            <span>API Secret:</span>
            <button 
              type="button"
              on:click={() => showApiSecret = !showApiSecret} 
              style="padding:4px 12px; font-size:11px; background:rgba(107, 114, 128, 0.3);"
            >
              {showApiSecret ? '🙈 Ẩn' : '👁️ Hiện'}
            </button>
          </div>
          <input 
            bind:value={apiSecret} 
            type={showApiSecret ? 'text' : 'password'}
            placeholder={credentialInfo ? 'Nhập API Secret mới để thay thế' : 'Nhập API Secret từ Binance'}
            style="width:100%; margin-top:8px; font-family: monospace; box-sizing:border-box;"
          />
        </label>

        <label style="margin-top:16px;">
          <span>Tên chủ TK ngân hàng (người chuyển):</span>
          <p style="color:#9ca3af; font-size:12px; margin:6px 0 0;">
            Nhập đúng chữ hoa/thường như trên thẻ/TK — nội dung CK và QR giữ nguyên:
            <code style="color:#fbbf24;">{'{tên} chuyen tien'}</code>
          </p>
          <div style="display:flex; gap:8px; margin-top:8px; align-items:center;">
            <input
              bind:value={payerBankName}
              type="text"
              placeholder="VD: NGUYEN VAN A hoặc Nguyen Van A"
              autocomplete="off"
              spellcheck="false"
              style="flex:1; font-family: monospace; box-sizing:border-box;"
            />
            {#if credentialsSaved}
              <button type="button" on:click={savePayerBankName} style="white-space:nowrap;">
                Lưu tên
              </button>
            {/if}
          </div>
        </label>

        <div style="margin-top:20px; display:flex; gap:12px; align-items:center; flex-wrap:wrap;">
          <button on:click={saveCreds} disabled={isSaving}>
            {#if isSaving}
              <span class="spinner"></span> Đang lưu...
            {:else}
              {credentialInfo ? '💾 Cập nhật Credentials' : '💾 Lưu Credentials'}
            {/if}
          </button>
          <button on:click={testCreds} disabled={isTesting || !credentialsSaved} style="background: linear-gradient(135deg, #10b981 0%, #059669 100%);">
            {#if isTesting}
              <span class="spinner"></span> Đang test...
            {:else}
              🔌 Test Kết Nối
            {/if}
          </button>
          {#if credentialInfo}
            <button on:click={removeCreds} style="background: rgba(107, 114, 128, 0.3); box-shadow:none;">
              🔓 Xoá Credentials
            </button>
          {/if}
        </div>
      </div>
      
      <div style="background: rgba(31, 41, 55, 0.8); padding: 24px; border-radius: 12px; border: 1px solid rgba(255, 255, 255, 0.05);">
        <h3>🔄 Đồng Bộ Dữ Liệu Từ Sàn</h3>
        <p style="color:#9ca3af; font-size:13px; margin-bottom:16px;">
          Tải toàn bộ lệnh giao dịch từ sàn Binance P2P về hệ thống
        </p>
        
        <label>
          Số ngày cần đồng bộ (1-30):
          <input 
            type="number" 
            bind:value={syncDays} 
            min="1" 
            max="30"
            style="width:120px; margin-top:8px;"
          />
        </label>
        
        <div style="margin-top:16px;">
          <button disabled={loading} on:click={doForceSync}>
            {#if loading}
              <span class="spinner"></span> Đang đồng bộ...
            {:else}
              🚀 Bắt Đầu Đồng Bộ
            {/if}
          </button>
        </div>
        
        {#if syncProgress}
          <div style="margin-top:16px; padding:12px; background:rgba(37, 99, 235, 0.1); border-left:4px solid #2563eb; border-radius:6px; color:#60a5fa; font-size:13px;">
            <div style="display:flex; align-items:center; gap:8px;">
              <span class="spinner"></span>
              <span>{syncProgress}</span>
            </div>
          </div>
        {/if}
        
        {#if loading}
          <div style="margin-top:16px; padding:12px; background:rgba(245, 158, 11, 0.1); border-left:4px solid #f59e0b; border-radius:6px; color:#fbbf24; font-size:13px;">
            ⚠️ <strong>Lưu ý:</strong> Quá trình đồng bộ có thể mất vài phút tùy thuộc vào số lượng lệnh. Vui lòng không đóng ứng dụng.
          </div>
        {/if}
      </div>
      
      {#if errorMsg}
        <div class="error" style="margin-top:16px;">{errorMsg}</div>
      {/if}
      
      <div style="margin-top:24px; padding:16px; background:rgba(59, 130, 246, 0.05); border:1px solid rgba(59, 130, 246, 0.2); border-radius:8px;">
        <h4 style="margin:0 0 8px 0; color:#60a5fa; font-size:14px;">💡 Hướng dẫn:</h4>
        <ul style="margin:0; padding-left:20px; font-size:13px; color:#9ca3af; line-height:1.8;">
          <li>Lấy API Key/Secret từ <a href="https://www.binance.com/en/my/settings/api-management" target="_blank" style="color:#60a5fa;">Binance API Management</a></li>
          <li>API Key/Secret được lưu trong kho khoá của Windows, không nằm trong file DB</li>
          <li>UI chỉ hiển thị API Key đã che; secret không bao giờ được đọc lại</li>
          <li>Luôn test kết nối trước khi đồng bộ dữ liệu</li>
          <li>Nếu gặp lỗi "Kết nối thất bại", kiểm tra lại API Key/Secret</li>
        </ul>
      </div>

      <!-- Clear All Data Section -->
      <div style="margin-top:24px; background: rgba(239, 68, 68, 0.1); padding: 24px; border-radius: 12px; border: 1px solid rgba(239, 68, 68, 0.3);">
        <h3 style="color:#fca5a5; margin-top:0;">🗑️ Xóa Toàn Bộ Dữ Liệu</h3>
        <p style="color:#9ca3af; font-size:13px; margin-bottom:16px;">
          Xóa toàn bộ lệnh, thông tin thanh toán và API credentials trong kho khoá hệ thống.
          App <strong>không</strong> cần tắt — dữ liệu được xoá ngay trong phiên hiện tại.
          <br/><strong style="color:#fca5a5;">Hành động này không thể hoàn tác!</strong>
        </p>
        
        {#if !showClearConfirm}
          <button 
            on:click={clearAllData}
            disabled={isClearing}
            style="background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%); box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);"
          >
            {#if isClearing}
              <span class="spinner"></span> Đang xóa...
            {:else}
              🗑️ Xóa Toàn Bộ Dữ Liệu
            {/if}
          </button>
        {:else}
          <div style="padding:16px; background:rgba(239, 68, 68, 0.2); border-radius:8px; border:2px solid #ef4444;">
            <p style="color:#fca5a5; font-weight:600; margin:0 0 16px 0;">
              ⚠️ BẠN CHẮC CHẮN MUỐN XÓA TOÀN BỘ DỮ LIỆU?
            </p>
            <p style="color:#9ca3af; font-size:13px; margin:0 0 8px 0;">
              Sẽ xóa:
            </p>
            <ul style="color:#9ca3af; font-size:13px; margin:0 0 16px 0; padding-left:24px;">
              <li>Tất cả lệnh giao dịch ({orders.length} lệnh)</li>
              <li>API credentials đã lưu</li>
              <li>Thông tin thanh toán</li>
            </ul>
            <p style="color:#fbbf24; font-size:13px; margin:0 0 16px 0; font-weight:500;">
              💡 Sau khi xoá, bạn có thể nhập lại API credentials và đồng bộ lại ngay.
            </p>
            <div style="display:flex; gap:12px;">
              <button 
                on:click={clearAllData}
                disabled={isClearing}
                style="background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%); box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);"
              >
                {#if isClearing}
                  <span class="spinner"></span> Đang xóa...
                {:else}
                  ✓ Đồng Ý Xóa
                {/if}
              </button>
              <button 
                on:click={cancelClear}
                disabled={isClearing}
                style="background: rgba(107, 114, 128, 0.3);"
              >
                ✗ Hủy Bỏ
              </button>
            </div>
            
            {#if isClearing}
              <div style="margin-top:16px; padding:12px; background:rgba(37, 99, 235, 0.1); border-left:4px solid #2563eb; border-radius:6px; color:#60a5fa; font-size:13px;">
                <div style="display:flex; align-items:center; gap:8px;">
                  <span class="spinner"></span>
                  <span>Đang xóa dữ liệu từ database... Vui lòng đợi</span>
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- Toast Notifications -->
<ToastContainer />

<!-- OrderDetail Modal -->
{#if selectedOrder}
  <OrderDetail order={selectedOrder} onClose={closeOrderDetail} />
{/if}

<!-- order table moved to lib/OrderTable.svelte -->
