<!-- Toast Notification Component -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  
  export let message: string = '';
  export let type: 'success' | 'error' | 'info' | 'warning' = 'info';
  export let duration: number = 3000;
  export let onClose: () => void;
  
  let visible = true;
  
  const icons = {
    success: '✓',
    error: '✕',
    info: 'ℹ',
    warning: '⚠'
  };
  
  const colors = {
    success: '#10b981',
    error: '#ef4444',
    info: '#3b82f6',
    warning: '#f59e0b'
  };
  
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    if (duration > 0) {
      hideTimer = setTimeout(() => {
        visible = false;
        closeTimer = setTimeout(onClose, 300);
      }, duration);
    }
    return () => {
      if (hideTimer) clearTimeout(hideTimer);
      if (closeTimer) clearTimeout(closeTimer);
    };
  });

  function handleClose() {
    if (hideTimer) clearTimeout(hideTimer);
    visible = false;
    closeTimer = setTimeout(onClose, 300);
  }
</script>

{#if visible}
  <div 
    class="toast"
    style="background: {colors[type]};"
    transition:fly="{{ y: -20, duration: 300 }}"
  >
    <div class="toast-icon">{icons[type]}</div>
    <div class="toast-message">{message}</div>
    <button class="toast-close" on:click={handleClose}>×</button>
  </div>
{/if}

<style>
  .toast {
    /* Không dùng fixed ở đây — ToastContainer đã xếp chồng các toast. */
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-radius: 8px;
    color: white;
    font-size: 14px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(10px);
    min-width: 300px;
    max-width: 500px;
    animation: slideIn 0.3s ease-out;
  }
  
  @keyframes slideIn {
    from {
      transform: translateX(400px);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
  
  .toast-icon {
    font-size: 20px;
    font-weight: bold;
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 50%;
  }
  
  .toast-message {
    flex: 1;
    line-height: 1.4;
  }
  
  .toast-close {
    background: transparent;
    border: none;
    color: white;
    font-size: 24px;
    cursor: pointer;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.7;
    transition: opacity 0.2s;
    flex-shrink: 0;
  }
  
  .toast-close:hover {
    opacity: 1;
  }
</style>
