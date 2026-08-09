// P2P Assistant Extension Popup

let statusEl;

document.addEventListener('DOMContentLoaded', () => {
    statusEl = document.getElementById('status');
    updateStatus('Extension loaded', true);
});



function updateStatus(message, connected) {
    if (statusEl) {
        statusEl.textContent = message;
        statusEl.className = `status ${connected ? 'connected' : 'disconnected'}`;
    }
}


