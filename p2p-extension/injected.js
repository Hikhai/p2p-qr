// @ts-nocheck
(function () {
  'use strict';

  const VERSION = '0.4.0';
  const DEBUG = true; // set true for limited console diagnostics
  
  console.log('[P2P Ext v' + VERSION + '] Injected script loaded, DEBUG=' + DEBUG);

  // Patterns/keywords checked against url.toLowerCase()
  const TARGET_PATTERNS = [
    '/bapi/c2c/',
    '/gateway-api/',
    '/sapi/v1/c2c/',
    '/c2c/',
    '/vi/fiatorderdetail'
  ];
  const RELEVANT_KEYWORDS = [
    'order', 'detail', 'payment', 'pay', 'payinfo', 'paymethod', 'bank', 'merchant', 'advertiser',
    'fiatorderdetail', 'orderdetail', 'order-match', 'matched'
  ];

  const sentUrls = new Set();

  function isTarget(url) {
    if (typeof url !== 'string') return false;
    const lower = url.toLowerCase();
    const hasPattern = TARGET_PATTERNS.some(p => lower.includes(p));
    if (!hasPattern) return false;
    const hasKeyword = RELEVANT_KEYWORDS.some(k => lower.includes(k));
    return hasKeyword;
  }

  function shouldSend(url) {
    const key = `${url}_${Date.now().toString().slice(0, -3)}`; // seconds bucket
    if (sentUrls.has(key)) return false;
    sentUrls.add(key);
    if (sentUrls.size > 50) {
      const oldEntries = Array.from(sentUrls).slice(0, 25);
      oldEntries.forEach(entry => sentUrls.delete(entry));
    }
    return true;
  }

  let diagCount = 0;
  function emit(obj) {
    try {
      if (!obj || !obj.url || !shouldSend(obj.url)) return;
      if (DEBUG && diagCount < 10) diagCount++;

      let paymentDetail = null;
      if (obj.data && obj.url) {
        const url = obj.url.toLowerCase();
        if (
          url.includes('detail') ||
          url.includes('payment') ||
          url.includes('payinfo') ||
          url.includes('advertiser') ||
          url.includes('order-match') ||
          url.includes('orderdetail') ||
          url.includes('fiatorder')
        ) {
          paymentDetail = extractPaymentDetails(obj.data, obj.url);
        }
      }

      window.postMessage({
        __P2P_CAPTURE__: {
          ...obj,
          userAgent: navigator.userAgent.slice(0, 80),
          timestamp: new Date().toISOString(),
          version: VERSION,
          paymentDetail
        }
      }, '*');
    } catch (_) {
      // silent
    }
  }

  function extractPaymentDetails(data, url) {
    try {
      if (!data || typeof data !== 'object') return null;
      
      const root = (data && typeof data === 'object' && data.data && typeof data.data === 'object') ? data.data : data;
      
      // ONLY extract payment for BUY orders (user pays seller)
      // SKIP SELL orders (user receives payment from buyer)
      const tradeType = root?.tradeType || root?.trade_type || data?.tradeType || data?.trade_type;
      if (tradeType && String(tradeType).toUpperCase() === 'SELL') {
        if (DEBUG) console.log('[P2P Extract] Skipping SELL order (no payment info needed)');
        return null;
      }
      
      // DEBUG: Log raw response để debug
      if (DEBUG) {
        console.log('[P2P Extract] Processing BUY order, extracting payment details');
        console.log('[P2P Extract] === FULL RESPONSE STRUCTURE ===');
        console.log('[P2P Extract] data:', JSON.stringify(data, null, 2).substring(0, 2000));
        console.log('[P2P Extract] root:', JSON.stringify(root, null, 2).substring(0, 2000));
        console.log('[P2P Extract] === END STRUCTURE ===');
      }
      
      const paymentInfo = {};

      // Order number from common places
      const orderSources = [
        data.orderNumber, data.order_number, data.orderId, data.order_id, data.orderNo, data.order_no,
        root?.orderNumber, root?.order_number, root?.orderNo, root?.order_no
      ];
      for (const orderNum of orderSources) {
        if (orderNum && (typeof orderNum === 'string' || typeof orderNum === 'number')) { 
          paymentInfo.orderNumber = String(orderNum); 
          break; 
        }
      }
      if (!paymentInfo.orderNumber && typeof url === 'string') {
        try {
          const u = new URL(url, window.location.origin);
          const qOrder = u.searchParams.get('orderNo') || u.searchParams.get('order_number') || u.searchParams.get('orderNumber');
          if (qOrder) paymentInfo.orderNumber = String(qOrder);
        } catch (_) {}
      }

      // Extract FIAT amount (VND, not USDT) - Priority: totalPrice
      const fiatAmountSources = [
        root?.totalPrice,      // This is the fiat amount (VND)
        root?.fiatAmount,      // Alternative fiat field
        root?.total,           // Another possible fiat field
        data?.totalPrice,
        data?.fiatAmount
      ];
      for (const amt of fiatAmountSources) {
        if (amt && (typeof amt === 'string' || typeof amt === 'number')) {
          const amtStr = String(amt);
          const amtNum = parseFloat(amtStr);
          // Only use if it looks like VND (large numbers > 100)
          if (!isNaN(amtNum) && amtNum > 100) {
            paymentInfo.amount = amtStr;
            if (DEBUG) console.log('[P2P Extract] Found FIAT amount:', amtStr);
            break;
          }
        }
      }
      
      // PRIORITY: Parse Binance order-match/order-detail response structure FIRST
      // Structure: data.payMethods[0].fields[] - EXTRACT BY FIELD NAME (DYNAMIC)
      // Use fieldName to identify the correct field regardless of order
      // This handles cases where different orders have different field arrangements
      // 
      // Fields to extract from data.payMethods[0].fields[] (by fieldName):
      // - "Họ và tên" / "Account Name" / fieldContentType:"payee" → accountName
      // - "Tên ngân hàng" / "Bank Name" → bankName
      // - "Số tài khoản" / "Account Number" / "Số thẻ" → accountNo
      // - "Chi nhánh" / "Branch" → branchName (optional)
      //
      // Fields to extract from root level (data.data):
      // - root.totalPrice = Số tiền (Amount in VND) - Fiat amount, not crypto
      // - root.refMessage = Nội dung chuyển khoản (Transfer Content) - PRIMARY source
      //
      // Backend will auto-generate:
      // - QR Code using VietQR API (bank BIN + account number)
      if (Array.isArray(root?.payMethods) && root.payMethods[0]) {
        const firstMethod = root.payMethods[0];
        
        if (Array.isArray(firstMethod?.fields)) {
          if (DEBUG) {
            console.log('[P2P Extract] === RAW FIELDS DUMP ===');
            firstMethod.fields.forEach((field, idx) => {
              console.log(`[P2P Extract] fields[${idx}]:`, {
                fieldName: field.fieldName,
                fieldValue: field.fieldValue,
                fieldContentType: field.fieldContentType
              });
            });
            console.log('[P2P Extract] === END RAW FIELDS ===');
          }
          
          // Loop through all fields and extract by fieldName (DYNAMIC EXTRACTION)
          for (let i = 0; i < firstMethod.fields.length; i++) {
            const field = firstMethod.fields[i];
            if (!field || !field.fieldValue) continue;
            
            const fieldName = String(field.fieldName || '').toLowerCase();
            const fieldValue = String(field.fieldValue).split('\n')[0].trim();
            const fieldContentType = String(field.fieldContentType || '').toLowerCase();
            
            if (!fieldValue) continue;
            
            // Account Name (Họ và tên)
            if (!paymentInfo.accountName && (
              fieldName.includes('họ và tên') || 
              fieldName.includes('ho va ten') ||
              fieldName.includes('Họ và tên') ||
              fieldName.includes('account name') ||
              fieldName.includes('Name') ||
              fieldName.includes('payee') ||
              fieldContentType === 'payee'
            )) {
              let accountName = fieldValue.replace(/Bank Card.*$/i, '').trim();
              if (accountName) {
                paymentInfo.accountName = accountName;
                if (DEBUG) console.log('[P2P Extract] Found accountName:', accountName);
              }
            }
            
            // Bank Name (Tên ngân hàng)
            else if (!paymentInfo.bankName && (
              fieldName.includes('tên ngân hàng') || 
              fieldName.includes('ten ngan hang') ||
              fieldName.includes('Tên ngân hàng') ||
              fieldName.includes('bank name') ||
              (fieldName.includes('ngân hàng') && !fieldName.includes('chi nhánh'))
            )) {
              paymentInfo.bankName = fieldValue;
              if (DEBUG) console.log('[P2P Extract] Found bankName:', fieldValue);
            }
            
            // Account Number (Số tài khoản/Số thẻ)
            else if (!paymentInfo.accountNo && (
              fieldName.includes('số tài khoản') || 
              fieldName.includes('so tai khoan') ||
              fieldName.includes('account number') ||
              fieldName.includes('số thẻ') ||
              fieldName.includes('so the') ||
              fieldName.includes('Số tài khoản/Số thẻ') ||

              fieldName.includes('card number')
            )) {
              let accountNo = fieldValue.replace(/Bank Card.*$/i, '').replace(/Account Number.*$/i, '').trim();
              if (accountNo) {
                paymentInfo.accountNo = accountNo;
                if (DEBUG) console.log('[P2P Extract] Found accountNo:', accountNo);
              }
            }
            
            // Branch (Chi nhánh)
            else if (!paymentInfo.branchName && (
              fieldName.includes('chi nhánh') || 
              fieldName.includes('chi nhanh') ||
              fieldName.includes('Chi nhánh mở tài khoản') ||
              fieldName.includes('branch')
            )) {
              if (fieldValue) {
                paymentInfo.branchName = fieldValue;
                if (DEBUG) console.log('[P2P Extract] Found branchName:', fieldValue);
              }
            }
          }
          
          // NOTE: Transfer Content (Nội dung chuyển khoản) extracted from root.refMessage below
          // NOTE: QR Code will be auto-generated by backend using VietQR
          
          // Also check method-level fields
          if (firstMethod.tradeMethodName && !paymentInfo.payMethodName) {
            paymentInfo.payMethodName = firstMethod.tradeMethodName;
          }
        }
      }
      
      // Extract transfer content/reference from root level (refMessage is the primary field)
      if (!paymentInfo.transferContent) {
        const transferSources = [
          root?.refMessage,           // PRIMARY: Binance uses refMessage for transfer content
          root?.transferContent, 
          root?.reference, 
          root?.memo, 
          root?.paymentReference, 
          root?.transferMemo,
          data?.refMessage,
          data?.transferContent, 
          data?.reference, 
          data?.memo
        ];
        for (const ref of transferSources) {
          if (ref && typeof ref === 'string' && ref.trim()) {
            paymentInfo.transferContent = ref.trim();
            if (DEBUG) console.log('[P2P Extract] Found transferContent:', ref.trim().substring(0, 50));
            break;
          }
        }
      }
      
      // Extract suggested transfer content
      if (!paymentInfo.suggestedTransferContent) {
        const suggestedSources = [
          root?.suggestedReference, root?.recommendedMemo, root?.suggestedTransferContent,
          data?.suggestedReference, data?.recommendedMemo
        ];
        for (const sug of suggestedSources) {
          if (sug && typeof sug === 'string' && sug.trim()) {
            paymentInfo.suggestedTransferContent = sug.trim();
            if (DEBUG) console.log('[P2P Extract] Found suggested content');
            break;
          }
        }
      }
      
      // OLD: Legacy fallback for other API response formats
      if (Array.isArray(root?.payMethods)) {
        for (const method of root.payMethods) {
          if (Array.isArray(method?.fields)) {
            for (const f of method.fields) {
              const t = String((f && (f.fieldContentType || f.fieldKey || f.name || '')) || '').toLowerCase();
              const rawVal = f && (f.fieldValue ?? f.value ?? f.val);
              const v = typeof rawVal === 'string' ? rawVal.trim() : (typeof rawVal === 'number' ? String(rawVal) : '');
              if (!v) continue;
              if (!paymentInfo.accountName && ((t.includes('account') && t.includes('name')) || t === 'payee' || t === 'account_name' || t === 'bankaccountname')) paymentInfo.accountName = v;
              else if (!paymentInfo.accountNo && ((t.includes('account') && (t.includes('no') || t.includes('number'))) || t === 'pay_account' || t === 'account_number' || t === 'bankaccountnumber')) paymentInfo.accountNo = v;
              else if (!paymentInfo.bankName && (t === 'bank' || (t.includes('bank') && t.includes('name')))) paymentInfo.bankName = v;
              else if (!paymentInfo.subBank && (t.includes('branch') || t === 'sub_bank' || t === 'subbank' || (t.includes('bank') && t.includes('branch')))) paymentInfo.subBank = v;
              else if (!paymentInfo.qrCodeUrl && (t.includes('qr') || t === 'qr_code' || t === 'qrcodeurl' || t === 'qrcodepath')) paymentInfo.qrCodeUrl = v;
              else if (!paymentInfo.transferContent && (t.includes('reference') || t.includes('memo') || t.includes('transfer') && t.includes('content'))) paymentInfo.transferContent = v;
              else if (!paymentInfo.suggestedTransferContent && (t.includes('suggested') || t.includes('recommended')) && (t.includes('reference') || t.includes('memo'))) paymentInfo.suggestedTransferContent = v;
            }
          }
        }
      }

      // Root-level common keys and variants
      if (typeof root?.payee === 'string' && root.payee) paymentInfo.accountName ||= root.payee;
      if (typeof root?.payAccount === 'string' && root.payAccount) paymentInfo.accountNo ||= root.payAccount;
      if (typeof root?.payBank === 'string' && root.payBank) paymentInfo.bankName ||= root.payBank;
      if (typeof root?.paySubBank === 'string' && root.paySubBank) paymentInfo.subBank ||= root.paySubBank;
      if (typeof root?.qrCodePath === 'string' && root.qrCodePath) paymentInfo.qrCodeUrl ||= root.qrCodePath;
      if (typeof root?.accountName === 'string') paymentInfo.accountName ||= root.accountName;
      if (typeof root?.bankAccountName === 'string') paymentInfo.accountName ||= root.bankAccountName;
      if (typeof root?.accountNo === 'string') paymentInfo.accountNo ||= root.accountNo;
      if (typeof root?.bankAccountNumber === 'string') paymentInfo.accountNo ||= root.bankAccountNumber;
      if (typeof root?.bankName === 'string') paymentInfo.bankName ||= root.bankName;
      if (typeof root?.branchName === 'string') paymentInfo.subBank ||= root.branchName;
      if (typeof root?.qrCodeUrl === 'string') paymentInfo.qrCodeUrl ||= root.qrCodeUrl;

      // Method label if present
      if (data.payMethodName || data.paymentMethodName) paymentInfo.payMethodName = data.payMethodName || data.paymentMethodName;

      // Fallback deep scan
      const checkFields = (obj) => {
        if (!obj || typeof obj !== 'object') return;
        for (const key of Object.keys(obj)) {
          const value = obj[key];
          const lowerKey = key.toLowerCase();
          if (typeof value === 'string') {
            if (!paymentInfo.accountName && lowerKey.includes('account') && lowerKey.includes('name')) paymentInfo.accountName = value;
            if (!paymentInfo.accountNo && (lowerKey === 'accountno' || lowerKey === 'accountnumber' || (lowerKey.includes('account') && (lowerKey.includes('no') || lowerKey.includes('number'))))) paymentInfo.accountNo = value;
            if (!paymentInfo.bankName && lowerKey.includes('bank') && lowerKey.includes('name')) paymentInfo.bankName = value;
            if (!paymentInfo.subBank && (lowerKey.includes('branch') || lowerKey.includes('sub_bank') || lowerKey.includes('subbank'))) paymentInfo.subBank = value;
            if (!paymentInfo.qrCodeUrl && lowerKey.includes('qr') && (lowerKey.includes('url') || lowerKey.includes('code'))) paymentInfo.qrCodeUrl = value;
            if (!paymentInfo.transferContent && (lowerKey.includes('reference') || lowerKey.includes('memo') || (lowerKey.includes('transfer') && lowerKey.includes('content')))) paymentInfo.transferContent = value;
            if (!paymentInfo.suggestedTransferContent && (lowerKey.includes('suggested') || lowerKey.includes('recommended')) && (lowerKey.includes('reference') || lowerKey.includes('memo'))) paymentInfo.suggestedTransferContent = value;
          }
          if (typeof value === 'string' || typeof value === 'number') {
            if (!paymentInfo.amount && (lowerKey === 'amount' || lowerKey === 'totalprice' || lowerKey === 'fiatamount' || lowerKey === 'price' || lowerKey === 'total')) {
              paymentInfo.amount = String(value);
            }
          }
          if (typeof value === 'object' && value !== null && !Array.isArray(value)) checkFields(value);
        }
      };
      checkFields(data);

      if (paymentInfo.orderNumber && (paymentInfo.accountName || paymentInfo.accountNo || paymentInfo.bankName || paymentInfo.subBank || paymentInfo.qrCodeUrl || paymentInfo.amount || paymentInfo.transferContent || paymentInfo.suggestedTransferContent)) {
        return paymentInfo;
      }
      return null;
    } catch (_) {
      return null;
    }
  }

  // Hook fetch
  const originalFetch = window.fetch;
  window.fetch = async function (...args) {
    const startTime = Date.now();
    try {
      const response = await originalFetch.apply(this, args);
      let url = args[0];
      if (url && typeof url === 'object' && 'url' in url) url = url.url;
      if (isTarget(url)) {
        if (DEBUG) console.log('[P2P Fetch] Intercepted:', url);
        const clone = response.clone();
        clone.json().then(data => {
          if (DEBUG) console.log('[P2P Fetch] Response data:', data);
          emit({ type: 'fetch', url, method: args[1]?.method || 'GET', status: response.status, duration: Date.now() - startTime, data });
        }).catch(() => {});
      }
      return response;
    } catch (error) {
      throw error;
    }
  };

  // Hook XHR
  const OriginalXHR = window.XMLHttpRequest;
  function PatchedXHR() {
    const xhr = new OriginalXHR();
    let requestUrl = '';
    let requestMethod = 'GET';
    let startTime = 0;
    const openOriginal = xhr.open;
    xhr.open = function (method, url, ...rest) {
      requestUrl = url;
      requestMethod = method;
      startTime = Date.now();
      return openOriginal.call(xhr, method, url, ...rest);
    };
    xhr.addEventListener('load', function () {
      try {
        if (isTarget(requestUrl)) {
          if (xhr.responseType === '' || xhr.responseType === 'text') {
            const responseText = xhr.responseText;
            if (responseText && responseText.trim().startsWith('{')) {
              try {
                const data = JSON.parse(responseText);
                emit({ type: 'xhr', url: requestUrl, method: requestMethod, status: xhr.status, duration: Date.now() - startTime, data });
              } catch (_) {}
            }
          }
        }
      } catch (_) {}
    });
    return xhr;
  }
  Object.setPrototypeOf(PatchedXHR.prototype, OriginalXHR.prototype);
  Object.setPrototypeOf(PatchedXHR, OriginalXHR);
  window.XMLHttpRequest = PatchedXHR;

})();
