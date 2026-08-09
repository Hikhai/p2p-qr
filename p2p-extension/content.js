// P2P Extension Content Script - Chrome Messaging Only
(function() {
	'use strict';
	
	const BRIDGE_VERSION = '1.0.1';
	const DEBUG = false;
	
	let isInjected = false;

	// Inject the script that hooks into page's network requests
	function injectScript() {
		if (isInjected) return;
		
		try {
			const script = document.createElement('script');
			script.src = chrome.runtime.getURL('injected.js');
			script.async = false;
			script.onload = () => {
				script.remove();
				isInjected = true;
				if (DEBUG) console.log('[P2P Ext] API interceptor loaded');
			};
			script.onerror = () => {
				// Failed to load injected script - silent handling
			};
			document.documentElement.appendChild(script);
		} catch (error) {
			// Inject script failed - silent handling
		}
	}

	// Listen for messages from injected script
	window.addEventListener('message', (event) => {
		// Only accept messages from same origin
		if (event.source !== window) return;
		
		const data = event.data;
		
		// Handle DOM scraper data
		if (data && data.__P2P_SCRAPE__) {
			try {
				if (DEBUG) console.log('[P2P Ext] Received scraped data:', data.__P2P_SCRAPE__);
				handlePaymentDetail(data.__P2P_SCRAPE__);
			} catch (error) {
				// Error processing scrape - silent handling
			}
			return;
		}
		
		// Handle API capture data (fallback)
		if (!data || !data.__P2P_CAPTURE__) return;

		try {
			if (DEBUG) console.log('[P2P Ext] Received data from injected script');
			
			// Forward network data if it contains payment details
			if (data.__P2P_CAPTURE__.paymentDetail) {
				handlePaymentDetail(data.__P2P_CAPTURE__.paymentDetail);
			}
			
		} catch (error) {
			// Error processing message - silent handling
		}
	});

	function handlePaymentDetail(paymentDetail) {
		try {
			// Gửi ngay khi có số lệnh + STK + ngân hàng. Amount/nội dung có thể đến
			// ở response sau; backend upsert bằng COALESCE nên không mất field cũ.
			const hasBasicInfo = paymentDetail.orderNumber &&
			                     paymentDetail.accountNo &&
			                     paymentDetail.bankName;

			if (!hasBasicInfo) {
				if (DEBUG) console.log('[P2P Ext] Skipping incomplete payment detail (waiting for full data)');
				return;
			}
			
			if (DEBUG) console.log('[P2P Ext] Processing complete payment detail:', {
				orderNumber: paymentDetail.orderNumber,
				hasAmount: !!paymentDetail.amount,
				hasTransferContent: !!paymentDetail.transferContent
			});
			
			// Send ONLY via background.js (has retry mechanism with queue)
			// Background.js will automatically retry if backend not ready
			if (chrome?.runtime?.sendMessage) {
				chrome.runtime.sendMessage({
					type: 'SAVE_PAYMENT_DETAIL',
					data: paymentDetail,
					timestamp: Date.now(),
					url: window.location.href,
					autoCapture: true
				}, (response) => {
					if (chrome.runtime.lastError) {
						if (DEBUG) console.error('[P2P Ext] Failed to send to background:', chrome.runtime.lastError);
						showNotification('❌ Extension error', 'error');
					} else if (response) {
						if (response.queued) {
							if (DEBUG) console.log('[P2P Ext] Payment queued, will retry when app ready');
							showNotification(`⏳ Payment queued for order ${paymentDetail.orderNumber}`, 'info');
						} else if (response.success) {
							if (DEBUG) console.log('[P2P Ext] Payment sent successfully');
							showNotification(`✅ Payment captured for order ${paymentDetail.orderNumber}`, 'success');
						} else {
							if (DEBUG) console.error('[P2P Ext] Backend error:', response);
							showNotification('❌ Failed to save payment', 'error');
						}
					}
				});
			} else {
				if (DEBUG) console.error('[P2P Ext] Chrome messaging API not available');
				showNotification('❌ Extension not ready', 'error');
			}
			
		} catch (error) {
			showNotification('❌ Failed to capture payment info', 'error');
		}
	}

	// Show in-page notification
	function showNotification(message, type = 'info') {
		const notification = document.createElement('div');
		notification.style.cssText = `
			position: fixed;
			top: 20px;
			right: 20px;
			z-index: 10000;
			padding: 12px 20px;
			border-radius: 8px;
			color: white;
			font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
			font-size: 14px;
			max-width: 350px;
			box-shadow: 0 4px 12px rgba(0,0,0,0.3);
			background: ${type === 'success' ? '#10b981' : type === 'error' ? '#ef4444' : '#3b82f6'};
		`;
		notification.textContent = message;
		
		document.body.appendChild(notification);
		
		// Auto-remove after 5 seconds
		setTimeout(() => {
			if (notification.parentNode) {
				notification.parentNode.removeChild(notification);
			}
		}, 5000);
	}

	// Initialize when DOM is ready
	function initialize() {
		if (DEBUG) console.log('[P2P Ext] Initializing content script on:', window.location.href);
		injectScript();
	}

	// Start initialization
	if (document.readyState === 'loading') {
		document.addEventListener('DOMContentLoaded', initialize);
	} else {
		initialize();
	}

})();