<!-- Copied from root src/routes/+page.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
  import { fade, fly } from 'svelte/transition';
  import OrderTable from '../lib/OrderTable.svelte';
  import OrderDetail from '../lib/OrderDetail.svelte';
  import ToastContainer, { toastSuccess, toastError, toast } from '../lib/ToastContainer.svelte';

  let apiKey = "";
  let apiSecret = "";
  let label = "default";
  let orders:any[] = [];
  let syncDays = 7;
  let activeTab:'dashboard'|'buy'|'sell'|'inprogress'|'settings' = 'dashboard';
  let loading = false;
  let errorMsg = "";
  let selectedOrder: any = null;
  let isAutoRefresh = true;
  let refreshing = false;
  let lastRefreshTime = 0;
  let showApiKey = false;
  let showApiSecret = false;
  let isSaving = false;
  let isTesting = false;
  let syncProgress = "";
  let credentialsSaved = false;
  let isClearing = false;
  let showClearConfirm = false;

  async function loadOrders(silent: boolean = false) {
    try { 
      const result = await invoke('list_orders_from_db', { limit: 0 }); 
      orders = result as any[];
      errorMsg = "";
    }
    catch (e:any) { 
      console.error('Error loading orders:', e);
      errorMsg = e.toString();
      if (!silent) {
        toastError('Không thể tải danh sách lệnh');
      }
    }
  }
  function fmtDate(ms?: number) {
    if (!ms) return '';
    try { return new Date(ms).toLocaleString('vi-VN'); } catch { return '' }
  }
  function partnerName(o:any) { return o.trade_type === 'BUY' ? o.seller_nickname : o.buyer_nickname }
  $: buyOrders = orders.filter(o=>o.trade_type==='BUY');
  $: sellOrders = orders.filter(o=>o.trade_type==='SELL');
  // in-progress status codes: 1 (Đang chờ thanh toán), 2 (Đã thanh toán), 3 (Đang xác minh)
  $: inProgressOrders = orders.filter(o=>o.status_code===1 || o.status_code===2 || o.status_code===3);
  $: lastSync = orders.reduce((m:number, o:any)=> Math.max(m, o.last_api_sync_ts||0), 0);
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
    const isUpdate = credentialsSaved; // Check if updating existing credentials
    
    try { 
      await invoke('store_api_credentials', { label, apiKey, apiSecret }); 
      errorMsg = "";
      credentialsSaved = true;
      toastSuccess(isUpdate ? '✅ Đã cập nhật API credentials' : '✅ Đã lưu API credentials thành công');
    }
    catch (e:any) { 
      errorMsg = e.toString();
      toastError('Lưu thất bại: ' + e.toString());
    }
    finally {
      isSaving = false;
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
      const res = await invoke<string>('test_api_credentials'); 
      errorMsg = "";
      toastSuccess('✅ Kết nối API thành công! Credentials hợp lệ.');
    }
    catch (e:any) { 
      errorMsg = "❌ Kết nối thất bại: " + e.toString();
      toastError('Kết nối thất bại - Kiểm tra lại API Key/Secret');
    }
    finally {
      isTesting = false;
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
      await invoke('force_initial_sync', { days: syncDays }); 
      
      syncProgress = "Đang xử lý và lưu dữ liệu...";
      await loadOrders();
      
      syncProgress = "";
      toastSuccess(`✅ Đã đồng bộ thành công ${orders.length} lệnh`);
      errorMsg = "";
    }
    catch (e:any) { 
      errorMsg = "❌ Đồng bộ thất bại: " + e.toString();
      syncProgress = "";
      toastError('Đồng bộ thất bại - Kiểm tra kết nối và credentials');
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
    
    console.log('[CLEAR_DATA] Starting clear operation...');
    toast('🗑️ Đang xóa dữ liệu... App sẽ tắt ngay sau đó.');
    
    try {
      console.log('[CLEAR_DATA] Calling clear_all_data...');
      
      // App will exit immediately, so this won't return
      await invoke<string>('clear_all_data');
      
      // This code won't execute because app exits
    } catch (err: any) {
      console.error('[CLEAR_DATA] Error:', err);
      errorMsg = `Lỗi khi xóa dữ liệu: ${err}`;
      toast(`❌ ${errorMsg}`);
      isClearing = false;
    }
  }
  
  function cancelClear() {
    showClearConfirm = false;
  }
  


  function handleOrderClick(order: any) {
    selectedOrder = order;
  }

  function closeOrderDetail() {
    selectedOrder = null;
  }

  async function refreshFromExchange(silent: boolean = false) {
    refreshing = true;
    errorMsg = "";
    try {
      await invoke('force_sync_recent');
      await loadOrders(true); // Silent reload to avoid toast spam
      lastRefreshTime = Date.now();
      if (!silent) {
        toastSuccess('Đã cập nhật từ sàn');
      }
    } catch (e: any) {
      errorMsg = e.toString();
      if (!silent) {
        toastError('Cập nhật thất bại');
      }
    } finally {
      refreshing = false;
    }
  }

  function toggleAutoRefresh() {
    isAutoRefresh = !isAutoRefresh;
  }

  function fmtTimeAgo(ms: number) {
    if (!ms) return 'Chưa bao giờ';
    const seconds = Math.floor((Date.now() - ms) / 1000);
    if (seconds < 60) return `${seconds}s trước`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m trước`;
    return `${Math.floor(seconds / 3600)}h trước`;
  }
  async function loadCredentials() {
    try {
      // Get saved credentials from database
      const credentials = await invoke<[string, string] | null>('get_saved_credentials');
      
      if (credentials) {
        // Credentials exist - fill the input fields
        const [savedApiKey, savedApiSecret] = credentials;
        apiKey = savedApiKey;
        apiSecret = savedApiSecret;
        credentialsSaved = true;
        console.log('[LOAD_CREDS] Loaded saved credentials');
      } else {
        // No credentials saved yet
        credentialsSaved = false;
        console.log('[LOAD_CREDS] No saved credentials found');
      }
    } catch (e) {
      console.error('[LOAD_CREDS] Error loading credentials:', e);
      credentialsSaved = false;
    }
  }

  onMount(() => {
    // fire and forget initial load
    loadOrders(true); // Silent initial load
    loadCredentials(); // Check if credentials exist
    
    let unlistenFn: UnlistenFn | undefined;
    listen('orders-updated', async (_e: Event<any>) => { await loadOrders(true); }) // Silent event reload
      .then((u: UnlistenFn) => { unlistenFn = u; })
      .catch(() => {});
    
    // Listen for payment details from extension
    const handleExtensionMessage = async (event: any) => {
      if (event.data.__TAURI_SAVE_PAYMENT__) {
        try {
          const paymentData = event.data.__TAURI_SAVE_PAYMENT__;
  
          
          const result = await invoke('save_payment_detail_from_extension', {
            orderNumber: paymentData.orderNumber,
            accountName: paymentData.accountName,
            accountNo: paymentData.accountNo,
            bankName: paymentData.bankName,
            subBank: paymentData.subBank,
            qrCodeUrl: paymentData.qrCodeUrl
          });
          
  
          await loadOrders(true); // Silent refresh to show updated data
        } catch (error) {
          // Handle silently
        }
      }
    };
    
    window.addEventListener('message', handleExtensionMessage);
    
    // Auto refresh every 30 seconds when enabled (silent mode)
    const interval = setInterval(async () => {
      if (isAutoRefresh && !refreshing) {
        await refreshFromExchange(true); // Silent refresh - no toast spam
      }
    }, 30000);

    return () => { 
      if (unlistenFn) unlistenFn(); 
      clearInterval(interval);
      window.removeEventListener('message', handleExtensionMessage);
    };
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
  font-size: 32px;
  opacity: 0.9;
}

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

.btn-auto-on {
  background: linear-gradient(135deg, #10b981 0%, #059669 100%);
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.3);
}

.btn-auto-on:hover {
  box-shadow: 0 6px 16px rgba(16, 185, 129, 0.4);
}

.btn-auto-off {
  background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
  box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
}

.btn-auto-off:hover {
  box-shadow: 0 6px 16px rgba(107, 114, 128, 0.4);
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
  <span style="margin-left:12px;opacity:.8;font-size:12px;">Đồng bộ cuối: {fmtDate(lastSync)}</span>
</nav>

{#if activeTab==='dashboard'}
  <div transition:fade>
    <h2>Tổng quan</h2>
    <div class="stats-grid">
      <div class="stat-card" transition:fly="{{ x: -20, delay: 0, duration: 300 }}">
        <div class="stat-icon">📊</div>
        <div class="stat-content">
          <div class="stat-label">Tất cả</div>
          <div class="stat-value">{orders.length}</div>
        </div>
      </div>
      
      <div class="stat-card buy-card" transition:fly="{{ x: -20, delay: 100, duration: 300 }}">
        <div class="stat-icon">🟢</div>
        <div class="stat-content">
          <div class="stat-label">Mua</div>
          <div class="stat-value buy">{buyOrders.length}</div>
        </div>
      </div>
      
      <div class="stat-card sell-card" transition:fly="{{ x: -20, delay: 200, duration: 300 }}">
        <div class="stat-icon">🔴</div>
        <div class="stat-content">
          <div class="stat-label">Bán</div>
          <div class="stat-value sell">{sellOrders.length}</div>
        </div>
      </div>
      
      <div class="stat-card progress-card" transition:fly="{{ x: -20, delay: 300, duration: 300 }}">
        <div class="stat-icon">⏳</div>
        <div class="stat-content">
          <div class="stat-label">Đang xử lý</div>
          <div class="stat-value progress">{inProgressOrders.length}</div>
        </div>
      </div>
    </div>
  </div>
  <div class="action-bar">
    <button class="btn-action" on:click={() => loadOrders()}>
      🔄 Tải lại
    </button>
    <button class="btn-action" on:click={() => refreshFromExchange()} disabled={refreshing}>
      {#if refreshing}
        <span class="spinner"></span> Đang cập nhật...
      {:else}
        📡 Cập nhật từ sàn
      {/if}
    </button>
    <button 
      class="btn-action" 
      class:btn-auto-on={isAutoRefresh}
      class:btn-auto-off={!isAutoRefresh}
      on:click={toggleAutoRefresh}
    >
      {isAutoRefresh ? '🔄 Tự động' : '⏸️ Thủ công'}
    </button>
    <span class="last-update">
      Cập nhật cuối: {fmtTimeAgo(lastRefreshTime)}
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
        <label>
          Label (Tên cấu hình):
          <input 
            bind:value={label} 
            placeholder="default" 
            disabled={credentialsSaved}
            style="width:100%; margin-top:8px; box-sizing:border-box; {credentialsSaved ? 'opacity:0.6; cursor:not-allowed;' : ''}"
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
            placeholder="Nhập API Key từ Binance"
            disabled={credentialsSaved}
            style="width:100%; margin-top:8px; font-family: monospace; box-sizing:border-box; {credentialsSaved ? 'opacity:0.6; cursor:not-allowed;' : ''}"
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
            placeholder="Nhập API Secret từ Binance"
            disabled={credentialsSaved}
            style="width:100%; margin-top:8px; font-family: monospace; box-sizing:border-box; {credentialsSaved ? 'opacity:0.6; cursor:not-allowed;' : ''}"
          />
        </label>
        
        {#if credentialsSaved}
          <div style="margin-top:16px; padding:12px; background:rgba(34, 197, 94, 0.1); border-left:4px solid #22c55e; border-radius:6px;">
            <p style="color:#4ade80; font-size:13px; margin:0;">
              ✓ Credentials đã được lưu. Để thay đổi, vui lòng xóa toàn bộ dữ liệu trước.
            </p>
          </div>
        {/if}
        
        <div style="margin-top:20px; display:flex; gap:12px; align-items:center;">
          <button 
            on:click={saveCreds} 
            disabled={isSaving || credentialsSaved}
            style="{credentialsSaved ? 'opacity:0.5; cursor:not-allowed;' : ''}"
          >
            {#if isSaving}
              <span class="spinner"></span> Đang lưu...
            {:else}
              💾 Lưu Credentials
            {/if}
          </button>
          <button on:click={testCreds} disabled={isTesting || !credentialsSaved} style="background: linear-gradient(135deg, #10b981 0%, #059669 100%);">
            {#if isTesting}
              <span class="spinner"></span> Đang test...
            {:else}
              🔌 Test Kết Nối
            {/if}
          </button>
          {#if credentialsSaved}
            <span style="color:#10b981; font-size:12px;">✅ Đã lưu</span>
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
          <li>API Key và Secret sẽ được ẩn bằng *** để bảo mật</li>
          <li>Click nút "👁️ Hiện" để xem lại thông tin đã nhập</li>
          <li>Luôn test kết nối trước khi đồng bộ dữ liệu</li>
          <li>Nếu gặp lỗi "Kết nối thất bại", kiểm tra lại API Key/Secret</li>
        </ul>
      </div>

      <!-- Clear All Data Section -->
      <div style="margin-top:24px; background: rgba(239, 68, 68, 0.1); padding: 24px; border-radius: 12px; border: 1px solid rgba(239, 68, 68, 0.3);">
        <h3 style="color:#fca5a5; margin-top:0;">🗑️ Xóa Toàn Bộ Dữ Liệu</h3>
        <p style="color:#9ca3af; font-size:13px; margin-bottom:16px;">
          Xóa file database <code style="background:rgba(0,0,0,0.3); padding:2px 6px; border-radius:4px;">p2p_app.db</code> để xóa tất cả dữ liệu. 
          <strong style="color:#fca5a5;">App sẽ tắt ngay lập tức. Vui lòng mở lại sau 2 giây.</strong><br/>
          <strong style="color:#fca5a5;">Hành động này không thể hoàn tác!</strong>
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
              💡 App sẽ tắt sau khi xóa. Hãy đợi 2 giây rồi mở lại thủ công.
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
