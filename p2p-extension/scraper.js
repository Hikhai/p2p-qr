// P2P Payment Detail Scraper - Extract from DOM instead of API
(function() {
  'use strict';
  
  console.log('[P2P Scraper] Initialized');
  
  let lastScrapeData = null;
  let lastScrapeTime = 0;
  const SCRAPE_COOLDOWN = 5000; // 5 seconds cooldown between scrapes
  
  function scrapePaymentDetails() {
    try {
      // Extract order number from URL
      const urlMatch = window.location.href.match(/orderNo=(\d+)/);
      if (!urlMatch) {
        console.log('[P2P Scraper] No order number in URL');
        return null;
      }
      
      const orderNumber = urlMatch[1];
      console.log('[P2P Scraper] Order number:', orderNumber);
      
      const paymentInfo = { orderNumber };
      
      // IMPROVED: Direct selector approach for Binance's payment detail page
      // Look for specific text nodes and extract the next sibling's value
      
      // Try to find payment detail container
      const allText = document.body.innerText;
      
      // Extract Name (Họ và tên)
      const nameMatch = allText.match(/(?:Name|Họ và tên)[:\s]*\n\s*([A-ZÀÁẠẢÃÂẦẤẬẨẪĂẰẮẶẲẴÈÉẸẺẼÊỀẾỆỂỄÌÍỊỈĨÒÓỌỎÕÔỒỐỘỔỖƠỜỚỢỞỠÙÚỤỦŨƯỪỨỰỬỮỲÝỴỶỸĐ\s]+)/i);
      if (nameMatch && nameMatch[1].trim() && !nameMatch[1].includes('Name') && !nameMatch[1].includes('Họ và tên')) {
        paymentInfo.accountName = nameMatch[1].trim();
      }
      
      // Extract Account Number (Số tài khoản)
      const accountMatch = allText.match(/(?:Bank Card\/Account Number|Số tài khoản\/Số thẻ|Số tài khoản)[:\s]*\n\s*([0-9]+)/i);
      if (accountMatch && accountMatch[1]) {
        paymentInfo.accountNo = accountMatch[1].trim();
      }
      
      // Extract Bank Name (Tên ngân hàng)
      const bankMatch = allText.match(/(?:Tên ngân hàng|Bank)[:\s]*\n\s*([A-Za-z0-9\s\-]+)/i);
      if (bankMatch && bankMatch[1].trim() && !bankMatch[1].includes('Tên ngân hàng') && !bankMatch[1].includes('Bank Card')) {
        paymentInfo.bankName = bankMatch[1].trim();
      }
      
      // Alternative: Try querySelector with more specific selectors
      if (!paymentInfo.accountName || !paymentInfo.accountNo || !paymentInfo.bankName) {
        // Binance uses specific class patterns, try to find value divs
        const detailRows = document.querySelectorAll('div[class*="row"], div[class*="item"], div[class*="field"]');
        
        detailRows.forEach(row => {
          const text = row.textContent || '';
          const children = Array.from(row.children);
          
          if (children.length >= 2) {
            const label = children[0].textContent.trim().toLowerCase();
            const value = children[children.length - 1].textContent.trim();
            
            // Skip if value looks like a label
            if (value.length > 50 || value.includes(':')) return;
            
            if (!paymentInfo.accountName && label.includes('name') || label.includes('họ')) {
              paymentInfo.accountName = value;
            } else if (!paymentInfo.accountNo && (label.includes('account') || label.includes('số tài'))) {
              paymentInfo.accountNo = value;
            } else if (!paymentInfo.bankName && label.includes('bank') || label.includes('ngân hàng')) {
              paymentInfo.bankName = value;
            }
          }
        });
      }
      
      // Extract QR code
      const qrImg = document.querySelector('img[alt*="QR"], img[src*="qr" i], img[src*="payment"]');
      if (qrImg) {
        paymentInfo.qrCodeUrl = qrImg.src;
      }
      
      // Extract FIAT amount (VND) - look for larger numbers typically with VND currency
      // Pattern 1: Look for "Số tiền:" label followed by VND amount
      const amountLabelMatch = document.body.textContent.match(/Số tiền[:\s]*([0-9,\.]+)\s*VND/i);
      if (amountLabelMatch) {
        paymentInfo.amount = amountLabelMatch[1].replace(/,/g, '');
      } else {
        // Pattern 2: Look for large numbers (VND amounts are typically > 100,000)
        const vndMatch = document.body.textContent.match(/([0-9,]+(?:\.[0-9]{1,2})?)\s*VND/i);
        if (vndMatch) {
          const amount = vndMatch[1].replace(/,/g, '');
          if (parseFloat(amount) > 100) { // Ensure it's a fiat amount, not crypto
            paymentInfo.amount = amount;
          }
        } else {
          // Pattern 3: Fallback to crypto amount (USDT) - but mark as crypto
          const cryptoMatch = document.body.textContent.match(/([0-9,]+\.[0-9]+)\s*(USDT|BTC|ETH)/i);
          if (cryptoMatch) {
            paymentInfo.cryptoAmount = cryptoMatch[1].replace(/,/g, '');
            paymentInfo.cryptoSymbol = cryptoMatch[2];
            // Note: Don't set paymentInfo.amount here - we want fiat amount only
          }
        }
      }
      
      console.log('[P2P Scraper] Extracted:', paymentInfo);
      
      // Check if data changed and cooldown period passed
      const now = Date.now();
      const dataStr = JSON.stringify(paymentInfo);
      
      if (dataStr === lastScrapeData && (now - lastScrapeTime) < SCRAPE_COOLDOWN) {
        console.log('[P2P Scraper] Skipped - duplicate data within cooldown period');
        return paymentInfo;
      }
      
      lastScrapeData = dataStr;
      lastScrapeTime = now;
      
      // Send to content script
      window.postMessage({
        __P2P_SCRAPE__: paymentInfo
      }, '*');
      
      return paymentInfo;
    } catch (e) {
      console.error('[P2P Scraper] Error:', e);
      return null;
    }
  }
  
  // Run scraper after page loads
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      setTimeout(scrapePaymentDetails, 2000);
    });
  } else {
    setTimeout(scrapePaymentDetails, 2000);
  }
  
  // Re-scrape on mutations (with debounce)
  let mutationTimeout = null;
  const observer = new MutationObserver(() => {
    if (window.location.href.includes('orderNo=')) {
      // Debounce mutations - only scrape once after 2 seconds of no changes
      clearTimeout(mutationTimeout);
      mutationTimeout = setTimeout(scrapePaymentDetails, 2000);
    }
  });
  
  observer.observe(document.body, {
    childList: true,
    subtree: false  // Changed to false to reduce sensitivity
  });
  
})();
