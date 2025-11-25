import { V as attr, W as ensure_array_like, X as attr_class, Y as bind_props, Z as attr_style, _ as stringify, $ as store_get, a0 as unsubscribe_stores } from "../../chunks/index2.js";
import { invoke } from "@tauri-apps/api/core";
import "@tauri-apps/api/event";
import { f as fallback } from "../../chunks/utils2.js";
import { a as ssr_context, e as escape_html } from "../../chunks/context.js";
import { w as writable } from "../../chunks/index.js";
function onDestroy(fn) {
  /** @type {SSRContext} */
  ssr_context.r.on_destroy(fn);
}
function Pagination($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let pages;
    let currentPage = fallback($$props["currentPage"], 1);
    let totalPages = fallback($$props["totalPages"], 1);
    let onPageChange = $$props["onPageChange"];
    function generatePageNumbers(current, total) {
      if (total <= 7) {
        return Array.from({ length: total }, (_, i) => i + 1);
      }
      const pages2 = [];
      pages2.push(1);
      if (current > 3) {
        pages2.push("...");
      }
      for (let i = Math.max(2, current - 1); i <= Math.min(total - 1, current + 1); i++) {
        pages2.push(i);
      }
      if (current < total - 2) {
        pages2.push("...");
      }
      if (total > 1) {
        pages2.push(total);
      }
      return pages2;
    }
    pages = generatePageNumbers(currentPage, totalPages);
    if (totalPages > 1) {
      $$renderer2.push("<!--[-->");
      $$renderer2.push(`<div class="pagination svelte-dlb7of"><button class="pagination-btn svelte-dlb7of"${attr("disabled", currentPage === 1, true)}>‹ Trước</button> <!--[-->`);
      const each_array = ensure_array_like(pages);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let page = each_array[$$index];
        if (page === "...") {
          $$renderer2.push("<!--[-->");
          $$renderer2.push(`<span class="pagination-ellipsis svelte-dlb7of">...</span>`);
        } else {
          $$renderer2.push("<!--[!-->");
          $$renderer2.push(`<button${attr_class("pagination-btn page-number svelte-dlb7of", void 0, { "active": page === currentPage })}>${escape_html(page)}</button>`);
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--> <button class="pagination-btn svelte-dlb7of"${attr("disabled", currentPage === totalPages, true)}>Sau ›</button></div>`);
    } else {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { currentPage, totalPages, onPageChange });
  });
}
function OrderTable($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let searchQueryLower, filteredList, totalPages, paginatedList;
    let list = fallback($$props["list"], () => [], true);
    let onOrderClick = fallback($$props["onOrderClick"], void 0);
    let itemsPerPage = fallback($$props["itemsPerPage"], 20);
    let currentPage = 1;
    let searchQuery = "";
    let filterStatus = "all";
    const nfFiat = new Intl.NumberFormat("vi-VN", { minimumFractionDigits: 0, maximumFractionDigits: 0 });
    function handlePageChange(page) {
      currentPage = page;
    }
    function formatAsset(value, asset) {
      if (!value) return "0";
      const num = typeof value === "string" ? parseFloat(value) : value;
      if (isNaN(num)) return "0";
      let digits = 8;
      if (asset === "USDT" || asset === "USDC" || asset === "BUSD") {
        digits = 2;
      } else if (asset === "BTC") {
        digits = 6;
      } else if (asset === "ETH") {
        digits = 4;
      }
      return new Intl.NumberFormat("vi-VN", { minimumFractionDigits: 0, maximumFractionDigits: digits }).format(num);
    }
    function fmtDate(ms) {
      if (!ms) return "";
      try {
        return new Date(ms).toLocaleString("vi-VN");
      } catch {
        return "";
      }
    }
    function partnerName(o) {
      return o.seller_nickname || o.buyer_nickname;
    }
    function toNum(s) {
      if (!s) return 0;
      const n = Number(s);
      return isFinite(n) ? n : 0;
    }
    function pricePerUnit(o) {
      const p = toNum(o.price);
      if (p > 0) return p;
      const total = toNum(o.total_fiat);
      const amt = toNum(o.amount_asset);
      if (total > 0 && amt > 0) return total / amt;
      return 0;
    }
    function statusText(o) {
      const label = (o.status_label || "").toString();
      if (label && !label.startsWith("Code")) return label;
      switch (o.status_code) {
        case 1:
          return "Đang chờ thanh toán";
        case 2:
          return "Đã thanh toán";
        case 3:
          return "Đang xác minh";
        case 4:
          return "Đã hoàn thành";
        case 5:
          return "Đã hủy";
        case 6:
          return "Đã hủy bởi hệ thống";
        default:
          return `Không xác định (${o.status_code})`;
      }
    }
    searchQueryLower = searchQuery.trim().toLowerCase();
    filteredList = list.filter((order) => {
      if (searchQueryLower) {
        const orderNum = order.order_number?.toString() || "";
        const seller = order.seller_nickname?.toLowerCase() || "";
        const buyer = order.buyer_nickname?.toLowerCase() || "";
        const asset = order.asset?.toLowerCase() || "";
        const fiat = order.fiat?.toLowerCase() || "";
        return orderNum.includes(searchQueryLower) || seller.includes(searchQueryLower) || buyer.includes(searchQueryLower) || asset.includes(searchQueryLower) || fiat.includes(searchQueryLower);
      }
      return true;
    });
    totalPages = Math.ceil(filteredList.length / itemsPerPage);
    if (filteredList.length > 0 && currentPage > totalPages) {
      currentPage = Math.max(1, totalPages);
    }
    {
      currentPage = 1;
    }
    paginatedList = filteredList.slice((currentPage - 1) * itemsPerPage, currentPage * itemsPerPage);
    if (
      // Mặc định
      // Stablecoin
      // BTC
      // ETH
      // Backend trả về status_label đã được mapping từ StageMap
      // Fallback với trạng thái chính xác như sàn Binance
      list.length === 0
    ) {
      $$renderer2.push("<!--[-->");
      $$renderer2.push(`<div class="empty-state svelte-1765f7f"><div class="empty-icon svelte-1765f7f">📭</div> <div class="empty-text svelte-1765f7f">Không có lệnh nào</div></div>`);
    } else {
      $$renderer2.push("<!--[!-->");
      $$renderer2.push(`<div class="filter-bar svelte-1765f7f"><div class="search-box svelte-1765f7f"><span class="search-icon svelte-1765f7f">🔍</span> <input type="text" placeholder="Tìm số lệnh, đối tác, asset..."${attr("value", searchQuery)} class="search-input svelte-1765f7f"/> `);
      {
        $$renderer2.push("<!--[!-->");
      }
      $$renderer2.push(`<!--]--></div> <div class="filter-buttons svelte-1765f7f"><button${attr_class("filter-btn svelte-1765f7f", void 0, { "active": filterStatus === "all" })}>Tất cả</button> <button${attr_class("filter-btn svelte-1765f7f", void 0, { "active": filterStatus === "inprogress" })}>Đang xử lý</button> <button${attr_class("filter-btn svelte-1765f7f", void 0, { "active": filterStatus === "completed" })}>Hoàn thành</button> <button${attr_class("filter-btn svelte-1765f7f", void 0, { "active": filterStatus === "cancelled" })}>Đã hủy</button> `);
      {
        $$renderer2.push("<!--[!-->");
      }
      $$renderer2.push(`<!--]--></div></div> <div class="table-info svelte-1765f7f"><span>Hiển thị ${escape_html(Math.min((currentPage - 1) * itemsPerPage + 1, filteredList.length))} - ${escape_html(Math.min(currentPage * itemsPerPage, filteredList.length))} trong tổng số ${escape_html(filteredList.length)} lệnh</span> `);
      if (filteredList.length < list.length) {
        $$renderer2.push("<!--[-->");
        $$renderer2.push(`<span class="filtered-badge svelte-1765f7f">(${escape_html(list.length - filteredList.length)} lệnh bị lọc)</span>`);
      } else {
        $$renderer2.push("<!--[!-->");
      }
      $$renderer2.push(`<!--]--></div> <div class="table-container svelte-1765f7f"><table class="svelte-1765f7f"><thead class="svelte-1765f7f"><tr><th class="svelte-1765f7f">Loại / Ngày tháng</th><th class="svelte-1765f7f">Số lệnh</th><th class="svelte-1765f7f">Giá</th><th class="svelte-1765f7f">Số tiền pháp định / Tiền mã hóa</th><th class="svelte-1765f7f">Đối tác</th><th class="svelte-1765f7f">Trạng thái</th></tr></thead><tbody><!--[-->`);
      const each_array = ensure_array_like(paginatedList);
      for (let i = 0, $$length = each_array.length; i < $$length; i++) {
        let o = each_array[i];
        $$renderer2.push(`<tr${attr_class("order-row svelte-1765f7f", void 0, { "clickable": onOrderClick })}${attr_style(`animation-delay: ${stringify(i * 30)}ms;`)}><td class="svelte-1765f7f"><div${attr_class("trade-type svelte-1765f7f", void 0, {
          "buy": o.trade_type === "BUY",
          "sell": o.trade_type === "SELL"
        })}>${escape_html(o.trade_type === "BUY" ? "🟢 MUA" : "🔴 BÁN")}</div> <div class="date-text svelte-1765f7f">${escape_html(fmtDate(o.create_time_ms))}</div></td><td class="order-number svelte-1765f7f">${escape_html(o.order_number)}</td><td class="price-cell svelte-1765f7f">${escape_html(nfFiat.format(pricePerUnit(o)))} <span class="fiat svelte-1765f7f">${escape_html(o.fiat)}</span></td><td class="svelte-1765f7f"><div class="amount-fiat svelte-1765f7f">${escape_html(nfFiat.format(toNum(o.total_fiat)))} ${escape_html(o.fiat)}</div> <div class="amount-crypto svelte-1765f7f">${escape_html(formatAsset(o.amount_asset, o.asset))} ${escape_html(o.asset)}</div></td><td class="partner-name svelte-1765f7f">${escape_html(partnerName(o) || "-")}</td><td${attr_class("status-cell status-" + statusText(o).replace(/\s+/g, "-"), "svelte-1765f7f")}>${escape_html(statusText(o))}</td></tr>`);
      }
      $$renderer2.push(`<!--]--></tbody></table></div> `);
      Pagination($$renderer2, { currentPage, totalPages, onPageChange: handlePageChange });
      $$renderer2.push(`<!---->`);
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { list, onOrderClick, itemsPerPage });
  });
}
function Toast($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let message = fallback($$props["message"], "");
    let type = fallback($$props["type"], "info");
    let duration = fallback($$props["duration"], 3e3);
    let onClose = $$props["onClose"];
    const icons = { success: "✓", error: "✕", info: "ℹ", warning: "⚠" };
    const colors = {
      success: "#10b981",
      error: "#ef4444",
      info: "#3b82f6",
      warning: "#f59e0b"
    };
    {
      $$renderer2.push("<!--[-->");
      $$renderer2.push(`<div class="toast svelte-1q6vvua"${attr_style(`background: ${stringify(colors[type])};`)}><div class="toast-icon svelte-1q6vvua">${escape_html(icons[type])}</div> <div class="toast-message svelte-1q6vvua">${escape_html(message)}</div> <button class="toast-close svelte-1q6vvua">×</button></div>`);
    }
    $$renderer2.push(`<!--]-->`);
    bind_props($$props, { message, type, duration, onClose });
  });
}
const toasts = writable([]);
function removeToast(id) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
function ToastContainer($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    $$renderer2.push(`<div class="toast-container svelte-1autuft"><!--[-->`);
    const each_array = ensure_array_like(store_get($$store_subs ??= {}, "$toasts", toasts));
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let toast = each_array[$$index];
      Toast($$renderer2, {
        message: toast.message,
        type: toast.type,
        duration: toast.duration || 3e3,
        onClose: () => removeToast(toast.id)
      });
    }
    $$renderer2.push(`<!--]--></div>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
function OrderDetail($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let sideRole;
    let order = $$props["order"];
    let onClose = $$props["onClose"];
    let paymentDetail = null;
    let loadingPaymentDetail = true;
    let copiedField = null;
    function fmtDate(ms) {
      if (!ms) return "Không có";
      try {
        return new Date(ms).toLocaleString("vi-VN", {
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit"
        });
      } catch (e) {
        return "Không hợp lệ";
      }
    }
    function formatNumber(value, digits = 0) {
      if (!value) return "0";
      const num = typeof value === "string" ? parseFloat(value) : value;
      if (isNaN(num)) return "0";
      return new Intl.NumberFormat("vi-VN", { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(num);
    }
    function formatCryptoAmount(value, asset) {
      if (!value) return "0";
      const num = typeof value === "string" ? parseFloat(value) : value;
      if (isNaN(num)) return "0";
      let digits = 8;
      if (asset === "USDT" || asset === "USDC" || asset === "BUSD") {
        digits = 2;
      } else if (asset === "BTC") {
        digits = 6;
      } else if (asset === "ETH") {
        digits = 4;
      }
      return new Intl.NumberFormat("vi-VN", { minimumFractionDigits: 0, maximumFractionDigits: digits }).format(num);
    }
    function getStatusColor(statusCode) {
      switch (statusCode) {
        case 1:
          return "#60a5fa";
        case // Đang chờ thanh toán
        2:
          return "#fbbf24";
        case // Đã thanh toán
        3:
          return "#f97316";
        case // Đang xác minh  
        4:
          return "#10b981";
        case // Đã hoàn thành
        5:
        case 6:
          return "#ef4444";
        default:
          return "#6b7280";
      }
    }
    async function loadPaymentDetail() {
      console.log("[DEBUG] loadPaymentDetail called for order:", order.order_number);
      loadingPaymentDetail = true;
      paymentDetail = null;
      try {
        const result = await invoke("get_order_payment_detail", { orderNumber: order.order_number });
        console.log("[DEBUG] Payment detail from DB:", result);
        if (result) {
          paymentDetail = result;
        } else {
          console.log("[DEBUG] Not in DB, checking localStorage...");
          await tryLoadFromLocalStorage();
        }
      } catch (error) {
        console.error("[DEBUG] Error loading payment details:", error);
      } finally {
        loadingPaymentDetail = false;
      }
    }
    async function tryLoadFromLocalStorage() {
      try {
        const key = `p2p_payment_${order.order_number}`;
        const stored = localStorage.getItem(key);
        if (stored) {
          console.log("[DEBUG] Found payment detail in localStorage");
          const data = JSON.parse(stored);
          try {
            await invoke("save_payment_detail_from_extension", {
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
            console.log("[DEBUG] Saved localStorage data to backend");
            const result = await invoke("get_order_payment_detail", { orderNumber: order.order_number });
            paymentDetail = result;
          } catch (saveError) {
            console.error("[DEBUG] Failed to save to backend:", saveError);
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
          console.log("[DEBUG] No payment detail in localStorage either");
        }
      } catch (error) {
        console.error("[DEBUG] Error accessing localStorage:", error);
      }
    }
    let lastLoadedOrderNumber = null;
    onDestroy(() => {
    });
    sideRole = order.trade_type === "BUY" ? "Mua" : "Bán";
    if (order?.order_number && order?.trade_type === "BUY" && order.order_number !== lastLoadedOrderNumber) {
      lastLoadedOrderNumber = order.order_number;
      loadPaymentDetail();
    }
    $$renderer2.push(`<div class="modal-overlay svelte-or40as"><div class="modal-content svelte-or40as"><div class="modal-header svelte-or40as"><h2 class="svelte-or40as">Chi tiết lệnh #${escape_html(
      // Live-refresh when extension pushes updates
      // If event source is extension and applies to this order, reload payment detail
      // Copy to clipboard function
      // Reset copied state after 2 seconds
      order.order_number
    )}</h2> <button class="close-btn svelte-or40as">×</button></div> <div class="modal-body svelte-or40as"><div class="detail-section svelte-or40as"><h3 class="svelte-or40as">Thông tin cơ bản</h3> <div class="detail-grid svelte-or40as"><div class="detail-item svelte-or40as"><span class="label svelte-or40as">Trạng thái lệnh:</span> <span class="value status svelte-or40as"${attr_style(`color: ${stringify(getStatusColor(order.status_code))}`)}>${escape_html(order.status_label || `Code-${order.status_code}`)}</span></div> <div class="detail-item svelte-or40as"><span class="label svelte-or40as">Loại lệnh:</span> <span${attr_class("value trade-type svelte-or40as", void 0, {
      "buy": order.trade_type === "BUY",
      "sell": order.trade_type === "SELL"
    })}>${escape_html(sideRole)} ${escape_html(order.asset)}</span></div> <div class="detail-item svelte-or40as"><span class="label svelte-or40as">Số tiền pháp định:</span> <span class="value amount svelte-or40as">${escape_html(formatNumber(order.total_fiat))} ${escape_html(order.fiat)}</span></div> <div class="detail-item svelte-or40as"><span class="label svelte-or40as">Giá:</span> <span class="value price svelte-or40as">${escape_html(formatNumber(order.price))} ${escape_html(order.fiat)}</span></div> <div class="detail-item svelte-or40as"><span class="label svelte-or40as">Số lượng ${escape_html(order.asset)}:</span> <span class="value crypto-amount svelte-or40as">${escape_html(formatCryptoAmount(order.amount_asset, order.asset))} ${escape_html(order.asset)}</span></div> <div class="detail-item svelte-or40as"><span class="label svelte-or40as">Thời gian tạo:</span> <span class="value time svelte-or40as">${escape_html(fmtDate(order.create_time_ms))}</span></div></div></div> `);
    if (order.trade_type === "BUY" && (order.status_code === 1 || order.status_code === 2 || order.status_code === 3)) {
      $$renderer2.push("<!--[-->");
      $$renderer2.push(`<div class="detail-section svelte-or40as"><h3 class="svelte-or40as">Thông tin thanh toán</h3> `);
      if (loadingPaymentDetail) {
        $$renderer2.push("<!--[-->");
        $$renderer2.push(`<div class="loading-state svelte-or40as"><span class="svelte-or40as">Đang tải thông tin thanh toán...</span></div>`);
      } else {
        $$renderer2.push("<!--[!-->");
        if (paymentDetail) {
          $$renderer2.push("<!--[-->");
          $$renderer2.push(`<div class="detail-grid svelte-or40as">`);
          if (paymentDetail.amount) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Số tiền:</span> <span class="value amount-highlight svelte-or40as">${escape_html(formatNumber(paymentDetail.amount))} VND</span></div> <button${attr_class("copy-btn svelte-or40as", void 0, { "copied": copiedField === "số tiền" })} title="Copy số tiền">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.transfer_content) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item highlight-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Nội dung chuyển khoản:</span> <span class="value transfer-content svelte-or40as">${escape_html(paymentDetail.transfer_content)}</span></div> <button${attr_class("copy-btn primary svelte-or40as", void 0, { "copied": copiedField === "nội dung CK" })} title="Copy nội dung chuyển khoản">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.account_name) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Họ và tên:</span> <span class="value svelte-or40as">${escape_html(paymentDetail.account_name)}</span></div> <button${attr_class("copy-btn svelte-or40as", void 0, { "copied": copiedField === "tên chủ TK" })} title="Copy tên chủ tài khoản">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.bank_name) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Tên ngân hàng:</span> <span class="value svelte-or40as">${escape_html(paymentDetail.bank_name)}</span></div> <button${attr_class("copy-btn svelte-or40as", void 0, { "copied": copiedField === "ngân hàng" })} title="Copy tên ngân hàng">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.account_no) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item highlight-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Số tài khoản/Số thẻ:</span> <span class="value account-number svelte-or40as">${escape_html(paymentDetail.account_no)}</span></div> <button${attr_class("copy-btn primary svelte-or40as", void 0, { "copied": copiedField === "số TK" })} title="Copy số tài khoản">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.sub_bank) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><div class="detail-content svelte-or40as"><span class="label svelte-or40as">Chi nhánh:</span> <span class="value svelte-or40as">${escape_html(paymentDetail.sub_bank)}</span></div> <button${attr_class("copy-btn svelte-or40as", void 0, { "copied": copiedField === "chi nhánh" })} title="Copy chi nhánh">`);
            {
              $$renderer2.push("<!--[!-->");
              $$renderer2.push(`📋`);
            }
            $$renderer2.push(`<!--]--></button></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.suggested_transfer_content) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><span class="label svelte-or40as">Nội dung chuyển khoản đề xuất:</span> <span class="value suggested-content svelte-or40as">${escape_html(paymentDetail.suggested_transfer_content)}</span></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.qr_code_url) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item qr-code-section svelte-or40as"><span class="label svelte-or40as">Mã QR:</span> <div class="qr-code-container svelte-or40as"><img${attr("src", paymentDetail.qr_code_url)} alt="QR Code thanh toán" class="qr-code-image svelte-or40as"/></div></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--> `);
          if (paymentDetail.captured_at) {
            $$renderer2.push("<!--[-->");
            $$renderer2.push(`<div class="detail-item svelte-or40as"><span class="label svelte-or40as">Thời gian cập nhật:</span> <span class="value svelte-or40as">${escape_html(fmtDate(paymentDetail.captured_at))}</span></div>`);
          } else {
            $$renderer2.push("<!--[!-->");
          }
          $$renderer2.push(`<!--]--></div>`);
        } else {
          $$renderer2.push("<!--[!-->");
          $$renderer2.push(`<div class="no-payment-info svelte-or40as"><span class="svelte-or40as">Chưa có thông tin thanh toán. Extension sẽ tự động cập nhật khi có dữ liệu từ network.</span></div>`);
        }
        $$renderer2.push(`<!--]-->`);
      }
      $$renderer2.push(`<!--]--></div>`);
    } else {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> <div class="detail-section svelte-or40as"><h3 class="svelte-or40as">Thông tin đối tác</h3> <div class="detail-grid svelte-or40as"><div class="detail-item svelte-or40as"><span class="label svelte-or40as">${escape_html(order.trade_type === "BUY" ? "Người bán" : "Người mua")}:</span> <span class="value svelte-or40as">${escape_html(order.trade_type === "BUY" ? order.seller_nickname : order.buyer_nickname)}</span></div></div></div></div> <div class="modal-footer svelte-or40as"><button class="btn-primary svelte-or40as">🌐 Mở trên Binance</button> <button class="btn-secondary svelte-or40as">Đóng</button></div></div></div>`);
    bind_props($$props, { order, onClose });
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let buyOrders, sellOrders, inProgressOrders, lastSync;
    let orders = [];
    let activeTab = "dashboard";
    let selectedOrder = null;
    let isAutoRefresh = true;
    let refreshing = false;
    function fmtDate(ms) {
      if (!ms) return "";
      try {
        return new Date(ms).toLocaleString("vi-VN");
      } catch {
        return "";
      }
    }
    function handleOrderClick(order) {
      selectedOrder = order;
    }
    function closeOrderDetail() {
      selectedOrder = null;
    }
    function fmtTimeAgo(ms) {
      return "Chưa bao giờ";
    }
    buyOrders = orders.filter((o) => o.trade_type === "BUY");
    sellOrders = orders.filter((o) => o.trade_type === "SELL");
    inProgressOrders = orders.filter((o) => o.status_code === 1 || o.status_code === 2 || o.status_code === 3);
    lastSync = orders.reduce((m, o) => Math.max(m, o.last_api_sync_ts || 0), 0);
    $$renderer2.push(`<nav class="svelte-1uha8ag"><button${attr("disabled", activeTab === "dashboard", true)} class="svelte-1uha8ag">Dashboard</button> <button${attr("disabled", activeTab === "buy", true)} class="svelte-1uha8ag">Lệnh mua</button> <button${attr("disabled", activeTab === "sell", true)} class="svelte-1uha8ag">Lệnh bán</button> <button${attr("disabled", activeTab === "inprogress", true)} class="svelte-1uha8ag">Đang xử lý</button> <button${attr("disabled", activeTab === "settings", true)} class="svelte-1uha8ag">Cài đặt</button> <span style="margin-left:12px;opacity:.8;font-size:12px;">Đồng bộ cuối: ${escape_html(fmtDate(lastSync))}</span></nav> `);
    {
      $$renderer2.push("<!--[-->");
      $$renderer2.push(`<div><h2 class="svelte-1uha8ag">Tổng quan</h2> <div class="stats-grid svelte-1uha8ag"><div class="stat-card svelte-1uha8ag"><div class="stat-icon svelte-1uha8ag">📊</div> <div class="stat-content svelte-1uha8ag"><div class="stat-label svelte-1uha8ag">Tất cả</div> <div class="stat-value svelte-1uha8ag">${escape_html(orders.length)}</div></div></div> <div class="stat-card buy-card svelte-1uha8ag"><div class="stat-icon svelte-1uha8ag">🟢</div> <div class="stat-content svelte-1uha8ag"><div class="stat-label svelte-1uha8ag">Mua</div> <div class="stat-value buy svelte-1uha8ag">${escape_html(buyOrders.length)}</div></div></div> <div class="stat-card sell-card svelte-1uha8ag"><div class="stat-icon svelte-1uha8ag">🔴</div> <div class="stat-content svelte-1uha8ag"><div class="stat-label svelte-1uha8ag">Bán</div> <div class="stat-value sell svelte-1uha8ag">${escape_html(sellOrders.length)}</div></div></div> <div class="stat-card progress-card svelte-1uha8ag"><div class="stat-icon svelte-1uha8ag">⏳</div> <div class="stat-content svelte-1uha8ag"><div class="stat-label svelte-1uha8ag">Đang xử lý</div> <div class="stat-value progress svelte-1uha8ag">${escape_html(inProgressOrders.length)}</div></div></div></div></div> <div class="action-bar svelte-1uha8ag"><button class="btn-action svelte-1uha8ag">🔄 Tải lại</button> <button class="btn-action svelte-1uha8ag"${attr("disabled", refreshing, true)}>`);
      {
        $$renderer2.push("<!--[!-->");
        $$renderer2.push(`📡 Cập nhật từ sàn`);
      }
      $$renderer2.push(`<!--]--></button> <button${attr_class("btn-action svelte-1uha8ag", void 0, { "btn-auto-on": isAutoRefresh, "btn-auto-off": !isAutoRefresh })}>${escape_html("🔄 Tự động")}</button> <span class="last-update svelte-1uha8ag">Cập nhật cuối: ${escape_html(fmtTimeAgo())}</span></div> `);
      {
        $$renderer2.push("<!--[!-->");
      }
      $$renderer2.push(`<!--]--> `);
      if (orders.length === 0) {
        $$renderer2.push("<!--[-->");
        $$renderer2.push(`<p style="color:#fbbf24; margin-top:10px;">Chưa có dữ liệu. Vào tab "Cài đặt" để cấu hình API và sync dữ liệu.</p>`);
      } else {
        $$renderer2.push("<!--[!-->");
        $$renderer2.push(`<div style="margin-top:16px;"><h3 class="svelte-1uha8ag">Lệnh mới nhất</h3> <p style="color:#9ca3af; font-size:12px; margin-bottom:8px;">💡 Click vào lệnh để xem chi tiết (trừ lệnh đang chờ thanh toán)</p> `);
        OrderTable($$renderer2, { list: orders.slice(0, 10), onOrderClick: handleOrderClick });
        $$renderer2.push(`<!----></div>`);
      }
      $$renderer2.push(`<!--]-->`);
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]--> `);
    ToastContainer($$renderer2);
    $$renderer2.push(`<!----> `);
    if (selectedOrder) {
      $$renderer2.push("<!--[-->");
      OrderDetail($$renderer2, { order: selectedOrder, onClose: closeOrderDetail });
    } else {
      $$renderer2.push("<!--[!-->");
    }
    $$renderer2.push(`<!--]-->`);
  });
}
export {
  _page as default
};
