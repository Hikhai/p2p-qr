// Background script for P2P Extension
// Handles communication between content script and local Tauri app (HTTP :1425)

console.log('[P2P Ext Background] Service worker started at', new Date().toISOString());

const SEND_COOLDOWN = 3000; // 3 seconds cooldown per order (successful sends)
const QUEUE_MAX_AGE_MS = 10 * 60 * 1000; // keep ORDER_NOT_FOUND retries up to 10 minutes
const QUEUE_MAX_ATTEMPTS = 40;

let lastSentData = {};
let requestQueue = [];
let isBackendReady = false;
let checkBackendInterval = null;

class BackendError extends Error {
    constructor(message, { status = 0, code = null, retryable = false } = {}) {
        super(message);
        this.name = 'BackendError';
        this.status = status;
        this.code = code;
        this.retryable = retryable;
    }
}

async function checkBackendHealth() {
    try {
        const response = await fetch('http://127.0.0.1:1425/api/health', {
            method: 'GET',
            signal: AbortSignal.timeout(1000)
        });
        return response.ok;
    } catch (e) {
        return false;
    }
}

function enqueue(bridgeData, orderNumber, reason) {
    const existing = requestQueue.find((item) => item.orderNumber === orderNumber);
    if (existing) {
        existing.data = bridgeData;
        existing.reason = reason;
        return;
    }
    requestQueue.push({
        data: bridgeData,
        orderNumber,
        reason,
        enqueuedAt: Date.now(),
        attempts: 0
    });
}

async function processQueue() {
    if (requestQueue.length === 0) return;

    const ready = await checkBackendHealth();
    if (!ready) {
        isBackendReady = false;
        return;
    }
    isBackendReady = true;

    const pending = requestQueue.splice(0, requestQueue.length);
    console.log(`[P2P Ext] Processing ${pending.length} queued request(s)`);

    for (const item of pending) {
        const age = Date.now() - item.enqueuedAt;
        if (age > QUEUE_MAX_AGE_MS || item.attempts >= QUEUE_MAX_ATTEMPTS) {
            console.warn(`[P2P Ext] Dropping queued order ${item.orderNumber} (age=${age}ms attempts=${item.attempts})`);
            continue;
        }

        try {
            item.attempts += 1;
            await sendToBackend(item.data);

            lastSentData[item.orderNumber] = {
                key: dataKeyFromBridge(item.data),
                timestamp: Date.now(),
                success: true
            };
            console.log(`[P2P Ext] Successfully sent queued request for order ${item.orderNumber}`);
        } catch (e) {
            if (e instanceof BackendError && e.retryable) {
                console.log(`[P2P Ext] Retry later for order ${item.orderNumber}: ${e.message}`);
                requestQueue.push(item);
            } else {
                console.error('[P2P Ext] Failed to send queued request:', e);
            }
        }
    }
}

function startBackendCheck() {
    if (checkBackendInterval) return;

    checkBackendInterval = setInterval(async () => {
        const ready = await checkBackendHealth();
        if (ready && !isBackendReady) {
            isBackendReady = true;
            console.log('[P2P Ext] Backend is now available');
        } else if (!ready && isBackendReady) {
            isBackendReady = false;
            console.log('[P2P Ext] Backend connection lost');
        }
        if (ready) {
            await processQueue();
        }
    }, 5000);
}

startBackendCheck();
console.log('[P2P Ext Background] Health check started (polling every 5 seconds)');

function dataKeyFromBridge(bridgeData) {
    return JSON.stringify({
        orderNumber: bridgeData.orderNumber,
        accountName: bridgeData.accountName,
        accountNo: bridgeData.accountNo,
        amount: bridgeData.amount,
        transferContent: bridgeData.transferContent,
        suggestedTransferContent: bridgeData.suggestedTransferContent
    });
}

async function sendToBackend(bridgeData) {
    let response;
    try {
        response = await fetch('http://127.0.0.1:1425/api/payment-detail', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                type: 'PAYMENT_DETAIL',
                data: {
                    orderNumber: bridgeData.orderNumber,
                    accountName: bridgeData.accountName,
                    accountNo: bridgeData.accountNo,
                    bankName: bridgeData.bankName,
                    subBank: bridgeData.subBank,
                    qrCodeUrl: bridgeData.qrCodeUrl,
                    amount: bridgeData.amount,
                    transferContent: bridgeData.transferContent,
                    suggestedTransferContent: bridgeData.suggestedTransferContent
                },
                timestamp: Date.now(),
                source: 'extension'
            }),
            signal: AbortSignal.timeout(5000)
        });
    } catch (e) {
        throw new BackendError(e.message || 'network error', { retryable: true });
    }

    let result = null;
    try {
        result = await response.json();
    } catch (_) {
        result = null;
    }

    if (!response.ok || result?.success === false) {
        const code = result?.code || null;
        const message = result?.message || `HTTP ${response.status}`;
        const retryable =
            code === 'ORDER_NOT_FOUND' ||
            response.status === 404 ||
            response.status >= 500;
        throw new BackendError(message, {
            status: response.status,
            code,
            retryable
        });
    }

    console.log('[P2P Ext] Payment detail sent to backend:', result);
    return result;
}

function createBridge(paymentDetail) {
    return {
        orderNumber: paymentDetail.orderNumber,
        accountName: paymentDetail.accountName,
        accountNo: paymentDetail.accountNo,
        bankName: paymentDetail.bankName,
        subBank: paymentDetail.branchName || paymentDetail.subBank,
        qrCodeUrl: paymentDetail.qrCodeUrl,
        amount: paymentDetail.amount,
        transferContent: paymentDetail.transferContent,
        suggestedTransferContent: paymentDetail.suggestedTransferContent,
        timestamp: Date.now(),
        captured: true
    };
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    console.log('[P2P Ext Background] Received message type:', message.type);

    if (message.type !== 'SAVE_PAYMENT_DETAIL') {
        return false;
    }

    const paymentDetail = message.data;
    console.log('[P2P Ext Background] Processing payment for order:', paymentDetail?.orderNumber);

    const orderNum = paymentDetail.orderNumber;
    const now = Date.now();
    const bridgeData = createBridge(paymentDetail);
    const dataKey = dataKeyFromBridge(bridgeData);

    if (lastSentData[orderNum]) {
        const timeDiff = now - lastSentData[orderNum].timestamp;
        const isSameData = lastSentData[orderNum].key === dataKey;
        if (lastSentData[orderNum].success && isSameData && timeDiff < SEND_COOLDOWN) {
            console.log(`[P2P Ext] Skipping duplicate request for order ${orderNum} (already sent ${timeDiff}ms ago)`);
            sendResponse({ success: true, cached: true });
            return true;
        }
    }

    if (requestQueue.some((item) => item.orderNumber === orderNum)) {
        // Refresh payload for the pending retry
        enqueue(bridgeData, orderNum, 'refresh');
        console.log(`[P2P Ext] Order ${orderNum} already queued for retry (payload refreshed)`);
        sendResponse({ success: true, queued: true });
        return true;
    }

    (async () => {
        try {
            await sendToBackend(bridgeData);
            lastSentData[orderNum] = {
                key: dataKey,
                timestamp: now,
                success: true
            };
            console.log(`[P2P Ext] Successfully sent payment detail for order ${orderNum}`);
            sendResponse({ success: true });
        } catch (error) {
            const retryable = error instanceof BackendError ? error.retryable : true;
            if (retryable) {
                console.log(`[P2P Ext] Queueing request for order ${orderNum}: ${error.message}`);
                enqueue(bridgeData, orderNum, error.code || 'retry');
                lastSentData[orderNum] = {
                    key: dataKey,
                    timestamp: now,
                    success: false
                };
                sendResponse({ success: true, queued: true });
            } else {
                console.error('[P2P Ext] Failed to process payment detail:', error);
                sendResponse({ success: false, error: error.message });
            }
        }
    })();

    return true;
});
