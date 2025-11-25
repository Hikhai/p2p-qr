<!-- Toast Container - Manages multiple toasts -->
<script lang="ts" context="module">
  import { writable } from 'svelte/store';
  
  export type ToastType = 'success' | 'error' | 'info' | 'warning';
  
  export interface ToastData {
    id: number;
    message: string;
    type: ToastType;
    duration?: number;
  }
  
  export const toasts = writable<ToastData[]>([]);
  
  let nextId = 1;
  
  export function showToast(message: string, type: ToastType = 'info', duration: number = 3000) {
    const id = nextId++;
    toasts.update(list => [...list, { id, message, type, duration }]);
  }
  
  export function removeToast(id: number) {
    toasts.update(list => list.filter(t => t.id !== id));
  }
  
  // Convenience functions
  export function toast(message: string) {
    showToast(message, 'info');
  }
  
  export function toastSuccess(message: string) {
    showToast(message, 'success');
  }
  
  export function toastError(message: string) {
    showToast(message, 'error', 5000); // Error stays longer
  }
  
  export function toastWarning(message: string) {
    showToast(message, 'warning');
  }
</script>

<script lang="ts">
  import Toast from './Toast.svelte';
</script>

<div class="toast-container">
  {#each $toasts as toast (toast.id)}
    <Toast
      message={toast.message}
      type={toast.type}
      duration={toast.duration || 3000}
      onClose={() => removeToast(toast.id)}
    />
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    top: 0;
    right: 0;
    z-index: 10000;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    pointer-events: none;
  }
  
  .toast-container :global(.toast) {
    pointer-events: all;
  }
</style>
