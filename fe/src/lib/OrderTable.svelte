<script lang="ts">
  import { fade, slide } from 'svelte/transition';
  import Pagination from './Pagination.svelte';
  
  export let list: any[] = [];
  export let onOrderClick: ((order: any) => void) | undefined = undefined;
  export let itemsPerPage: number = 20;
  
  let currentPage = 1;
  let searchQuery = '';
  let filterStatus: string = 'all';
  
  const nfFiat = new Intl.NumberFormat('vi-VN', { minimumFractionDigits: 0, maximumFractionDigits: 0 });
  
  // Optimize: Only compute search query lowercase once
  $: searchQueryLower = searchQuery.trim().toLowerCase();
  
  // Filter and search logic - optimized
  $: filteredList = list.filter(order => {
    // Status filter (fast check first)
    if (filterStatus !== 'all') {
      const statusCode = order.status_code;
      if (filterStatus === 'inprogress' && ![1, 2, 3].includes(statusCode)) {
        return false;
      } else if (filterStatus === 'completed' && statusCode !== 4) {
        return false;
      } else if (filterStatus === 'cancelled' && ![5, 6].includes(statusCode)) {
        return false;
      }
    }
    
    // Search filter (skip if no search query)
    if (searchQueryLower) {
      // Cache toLowerCase() results
      const orderNum = order.order_number?.toString() || '';
      const seller = order.seller_nickname?.toLowerCase() || '';
      const buyer = order.buyer_nickname?.toLowerCase() || '';
      const asset = order.asset?.toLowerCase() || '';
      const fiat = order.fiat?.toLowerCase() || '';
      
      return (
        orderNum.includes(searchQueryLower) ||
        seller.includes(searchQueryLower) ||
        buyer.includes(searchQueryLower) ||
        asset.includes(searchQueryLower) ||
        fiat.includes(searchQueryLower)
      );
    }
    
    return true;
  });
  
  // Pagination logic
  $: totalPages = Math.ceil(filteredList.length / itemsPerPage);
  $: paginatedList = filteredList.slice((currentPage - 1) * itemsPerPage, currentPage * itemsPerPage);
  $: if (filteredList.length > 0 && currentPage > totalPages) {
    currentPage = Math.max(1, totalPages);
  }
  
  // Reset to page 1 when filters change
  $: if (searchQueryLower || filterStatus) {
    currentPage = 1;
  }
  
  function handlePageChange(page: number) {
    currentPage = page;
  }
  
  function clearFilters() {
    searchQuery = '';
    filterStatus = 'all';
  }
  
  function formatAsset(value: string | number, asset: string) {
    if (!value) return '0';
    const num = typeof value === 'string' ? parseFloat(value) : value;
    if (isNaN(num)) return '0';
    
    let digits = 8; // Mặc định
    if (asset === 'USDT' || asset === 'USDC' || asset === 'BUSD') {
      digits = 2; // Stablecoin
    } else if (asset === 'BTC') {
      digits = 6; // BTC
    } else if (asset === 'ETH') {
      digits = 4; // ETH
    }
    
    return new Intl.NumberFormat('vi-VN', { 
      minimumFractionDigits: 0,
      maximumFractionDigits: digits 
    }).format(num);
  }
  function fmtDate(ms?: number) {
    if (!ms) return '';
    try { return new Date(ms).toLocaleString('vi-VN'); } catch { return '' }
  }
  function partnerName(o:any) { return o.seller_nickname || o.buyer_nickname }
  function toNum(s?: string) { if (!s) return 0; const n = Number(s); return isFinite(n) ? n : 0; }
  function pricePerUnit(o:any) {
    const p = toNum(o.price);
    if (p > 0) return p;
    const total = toNum(o.total_fiat);
    const amt = toNum(o.amount_asset);
    if (total > 0 && amt > 0) return total / amt;
    return 0;
  }
  function statusText(o:any) {
    // Backend trả về status_label đã được mapping từ StageMap
    const label = (o.status_label || '').toString();
    if (label && !label.startsWith('Code')) return label;
    
    // Fallback với trạng thái chính xác như sàn Binance
    switch (o.status_code) {
      case 1: return 'Đang chờ thanh toán';
      case 2: return 'Đã thanh toán';
      case 3: return 'Đang xác minh';
      case 4: return 'Đã hoàn thành';
      case 5: return 'Đã hủy';
      case 6: return 'Đã hủy bởi hệ thống';
      default: return `Không xác định (${o.status_code})`;
    }
  }
</script>

{#if list.length === 0}
  <div class="empty-state" transition:fade>
    <div class="empty-icon">📭</div>
    <div class="empty-text">Không có lệnh nào</div>
  </div>
{:else}
  <!-- Search and Filter Bar -->
  <div class="filter-bar" transition:fade>
    <div class="search-box">
      <span class="search-icon">🔍</span>
      <input 
        type="text" 
        placeholder="Tìm số lệnh, đối tác, asset..."
        bind:value={searchQuery}
        class="search-input"
      />
      {#if searchQuery}
        <button class="clear-search" on:click={() => searchQuery = ''}>×</button>
      {/if}
    </div>
    
    <div class="filter-buttons">
      <button 
        class="filter-btn"
        class:active={filterStatus === 'all'}
        on:click={() => filterStatus = 'all'}
      >
        Tất cả
      </button>
      <button 
        class="filter-btn"
        class:active={filterStatus === 'inprogress'}
        on:click={() => filterStatus = 'inprogress'}
      >
        Đang xử lý
      </button>
      <button 
        class="filter-btn"
        class:active={filterStatus === 'completed'}
        on:click={() => filterStatus = 'completed'}
      >
        Hoàn thành
      </button>
      <button 
        class="filter-btn"
        class:active={filterStatus === 'cancelled'}
        on:click={() => filterStatus = 'cancelled'}
      >
        Đã hủy
      </button>
      
      {#if searchQuery || filterStatus !== 'all'}
        <button class="clear-filters" on:click={clearFilters}>
          ✕ Xóa bộ lọc
        </button>
      {/if}
    </div>
  </div>
  
  <div class="table-info" transition:fade>
    <span>Hiển thị {Math.min((currentPage - 1) * itemsPerPage + 1, filteredList.length)} - {Math.min(currentPage * itemsPerPage, filteredList.length)} trong tổng số {filteredList.length} lệnh</span>
    {#if filteredList.length < list.length}
      <span class="filtered-badge">({list.length - filteredList.length} lệnh bị lọc)</span>
    {/if}
  </div>

  <div class="table-container">
    <table>
      <thead>
        <tr>
          <th>Loại / Ngày tháng</th>
          <th>Số lệnh</th>
          <th>Giá</th>
          <th>Số tiền pháp định / Tiền mã hóa</th>
          <th>Đối tác</th>
          <th>Trạng thái</th>
        </tr>
      </thead>
      <tbody>
        {#each paginatedList as o, i (o.order_number)}
          <tr class="order-row" 
              class:clickable={onOrderClick}
              style="animation-delay: {i * 30}ms;"
              transition:slide="{{ duration: 200 }}"
              on:click={() => {
                if (onOrderClick) {
                  onOrderClick(o);
                }
              }}>
            <td>
              <div class="trade-type" class:buy={o.trade_type === 'BUY'} class:sell={o.trade_type === 'SELL'}>
                {o.trade_type === 'BUY' ? '🟢 MUA' : '🔴 BÁN'}
              </div>
              <div class="date-text">{fmtDate(o.create_time_ms)}</div>
            </td>
            <td class="order-number">{o.order_number}</td>
            <td class="price-cell">{nfFiat.format(pricePerUnit(o))} <span class="fiat">{o.fiat}</span></td>
            <td>
              <div class="amount-fiat">{nfFiat.format(toNum(o.total_fiat))} {o.fiat}</div>
              <div class="amount-crypto">{formatAsset(o.amount_asset, o.asset)} {o.asset}</div>
            </td>
            <td class="partner-name">{partnerName(o) || '-'}</td>
            <td class={"status-cell status-"+statusText(o).replace(/\s+/g, '-')}>{statusText(o)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <Pagination 
    currentPage={currentPage}
    totalPages={totalPages}
    onPageChange={handlePageChange}
  />
{/if}

<style>
  .empty-state {
    padding: 60px 20px;
    text-align: center;
    color: #6b7280;
  }
  
  .empty-icon {
    font-size: 64px;
    margin-bottom: 16px;
    opacity: 0.5;
  }
  
  .empty-text {
    font-size: 16px;
    font-weight: 500;
  }
  
  .filter-bar {
    background: rgba(31, 41, 55, 0.6);
    backdrop-filter: blur(10px);
    padding: 16px;
    border-radius: 8px;
    margin-bottom: 16px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    overflow: hidden;
  }
  
  .search-box {
    position: relative;
    margin-bottom: 12px;
    overflow: hidden;
  }
  
  .search-icon {
    position: absolute;
    left: 14px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 16px;
    opacity: 0.6;
  }
  
  .search-input {
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    padding: 12px 40px 12px 44px;
    background: #111827;
    border: 1px solid #374151;
    border-radius: 8px;
    color: #e5e7eb;
    font-size: 14px;
    transition: all 0.2s ease;
  }
  
  .search-input:focus {
    outline: none;
    border-color: #2563eb;
    box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
  }
  
  .search-input::placeholder {
    color: #6b7280;
  }
  
  .clear-search {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    background: rgba(239, 68, 68, 0.2);
    border: none;
    color: #ef4444;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
  }
  
  .clear-search:hover {
    background: rgba(239, 68, 68, 0.3);
  }
  
  .filter-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  
  .filter-btn {
    background: rgba(55, 65, 81, 0.5);
    color: #9ca3af;
    border: 1px solid #374151;
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s ease;
    font-weight: 500;
  }
  
  .filter-btn:hover {
    background: rgba(55, 65, 81, 0.8);
    border-color: #4b5563;
  }
  
  .filter-btn.active {
    background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
    color: white;
    border-color: #2563eb;
    box-shadow: 0 4px 12px rgba(37, 99, 235, 0.3);
  }
  
  .clear-filters {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s ease;
    font-weight: 500;
  }
  
  .clear-filters:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.5);
  }
  
  .table-info {
    font-size: 13px;
    color: #9ca3af;
    margin-bottom: 12px;
    padding: 8px 12px;
    background: rgba(31, 41, 55, 0.5);
    border-radius: 6px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  
  .filtered-badge {
    background: rgba(251, 191, 36, 0.15);
    color: #fbbf24;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 600;
  }
  
  .table-container {
    overflow-x: auto;
    border-radius: 8px;
    border: 1px solid #374151;
    background: #1f2937;
    
    /* Hide scrollbar but keep functionality */
    scrollbar-width: none; /* Firefox */
    -ms-overflow-style: none; /* IE and Edge */
  }
  
  .table-container::-webkit-scrollbar {
    display: none; /* Chrome, Safari, Opera */
  }
  
  table { 
    border-collapse: collapse; 
    width: 100%; 
    font-size: 13px;
  }
  
  th, td { 
    border: 1px solid #374151; 
    padding: 12px 14px;
    text-align: left;
  }
  
  thead {
    background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
    position: sticky;
    top: 0;
    z-index: 10;
  }
  
  th {
    color: #e2e8f0;
    font-weight: 600;
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.5px;
  }
  
  .order-row {
    animation: fadeInRow 0.3s ease-out forwards;
    opacity: 0;
    transition: all 0.2s ease;
  }
  
  @keyframes fadeInRow {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  
  .order-row.clickable { 
    cursor: pointer;
  }
  
  .order-row.clickable:hover { 
    background: linear-gradient(90deg, rgba(37, 99, 235, 0.08) 0%, rgba(31, 41, 55, 0.3) 100%);
    transform: translateX(4px);
    box-shadow: inset 4px 0 0 #2563eb;
  }
  
  .trade-type {
    font-weight: 700;
    font-size: 12px;
    padding: 4px 8px;
    border-radius: 4px;
    display: inline-block;
    margin-bottom: 4px;
  }
  
  .trade-type.buy {
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }
  
  .trade-type.sell {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
  }
  
  .date-text {
    opacity: 0.7;
    font-size: 11px;
    color: #9ca3af;
  }
  
  .order-number {
    font-family: 'Courier New', monospace;
    color: #60a5fa;
    font-size: 12px;
  }
  
  .price-cell {
    font-weight: 600;
    color: #fbbf24;
  }
  
  .price-cell .fiat {
    color: #9ca3af;
    font-size: 11px;
    font-weight: 400;
  }
  
  .amount-fiat {
    color: #10b981;
    font-weight: 600;
    margin-bottom: 4px;
  }
  
  .amount-crypto {
    color: #60a5fa;
    font-size: 12px;
  }
  
  .partner-name {
    color: #e5e7eb;
  }
  
  .status-cell {
    font-weight: 600;
    padding: 6px 10px !important;
    border-radius: 6px;
    font-size: 12px;
    text-align: center;
  }
  
  /* Trạng thái theo Binance P2P */
  .status-Đang-chờ-thanh-toán { 
    color: #60a5fa; 
    background: rgba(37, 99, 235, 0.15);
    border: 1px solid rgba(37, 99, 235, 0.3);
  }
  
  .status-Đã-thanh-toán { 
    color: #fbbf24; 
    background: rgba(251, 191, 36, 0.15);
    border: 1px solid rgba(251, 191, 36, 0.3);
  }
  
  .status-Đang-xác-minh { 
    color: #f97316; 
    background: rgba(249, 115, 22, 0.15);
    border: 1px solid rgba(249, 115, 22, 0.3);
  }
  
  .status-Đã-hoàn-thành { 
    color: #10b981; 
    background: rgba(16, 185, 129, 0.15);
    border: 1px solid rgba(16, 185, 129, 0.3);
  }
  
  .status-Đã-hủy, .status-Đã-hủy-bởi-hệ-thống { 
    color: #ef4444; 
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
  }
  
  .status-Không-xác-định { 
    color: #6b7280; 
    background: rgba(107, 114, 128, 0.15);
    border: 1px solid rgba(107, 114, 128, 0.3);
  }
  
  /* Backup cho code numbers */
  .status-Code-1 { color: #60a5fa; }
  .status-Code-2 { color: #fbbf24; }
  .status-Code-3 { color: #f97316; }
  .status-Code-4 { color: #10b981; }
  .status-Code-5, .status-Code-6 { color: #ef4444; }
  
  @media (max-width: 768px) {
    table {
      font-size: 11px;
    }
    
    th, td {
      padding: 8px 10px;
    }
    
    .trade-type {
      font-size: 10px;
      padding: 3px 6px;
    }
  }
</style>
