<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { toastSuccess, toastError } from './ToastContainer.svelte';

  export type BotConfig = {
    bankName: string;
    accountNo: string;
    accountName: string;
    qrTransferContent: string;
    pollIntervalSecs: number;
    orderMaxAgeMinutes: number;
    greetingMessage: string;
    instructionMessage: string;
  };

  type BotStatus = 'idle' | 'running' | 'stopping';
  type EditSection = null | 'messages' | 'qr' | 'backup' | 'timing';

  /** Để trống = nội dung CK mặc định của ngân hàng (không nhúng addInfo vào QR). */
  const BANK_DEFAULT_QR = '';

  let cfg: BotConfig = {
    bankName: '',
    accountNo: '',
    accountName: '',
    qrTransferContent: BANK_DEFAULT_QR,
    pollIntervalSecs: 10,
    orderMaxAgeMinutes: 30,
    greetingMessage: '',
    instructionMessage: '',
  };

  /** Checkbox riêng — không suy ra từ ô trống (tránh mất ô khi đang xoá để gõ lại). */
  let useBankDefaultQr = true;

  $: qrPreview = useBankDefaultQr
    ? 'Mặc định ngân hàng'
    : (cfg.qrTransferContent || '').trim() || 'Tuỳ chỉnh (chưa nhập)';

  let status: BotStatus = 'idle';
  let logs: string[] = [];
  let editSection: EditSection = null;
  let autoScroll = true;
  let saving = false;
  let logBox: HTMLDivElement | null = null;

  $: busy = status !== 'idle';
  $: running = status === 'running';
  $: greetingPreview = (cfg.greetingMessage || '')
    .split('\n')
    .map((l) => l.trim())
    .find(Boolean) || 'Chưa có tin chào';

  const unsubs: UnlistenFn[] = [];

  function statusLabel(s: BotStatus) {
    if (s === 'running') return 'Đang chạy';
    if (s === 'stopping') return 'Đang dừng…';
    return 'Đã dừng';
  }

  function statusHint(s: BotStatus) {
    if (s === 'running')
      return 'Tin chào + QR khi có lệnh · tin cảm ơn khi giao dịch xong';
    if (s === 'stopping') return 'Đang kết thúc phiên…';
    return 'Bấm Bắt đầu khi sẵn sàng. API lấy từ tab Cài đặt.';
  }

  function onBankDefaultQrChange(e: Event) {
    useBankDefaultQr = (e.currentTarget as HTMLInputElement).checked;
    if (useBankDefaultQr) {
      cfg.qrTransferContent = BANK_DEFAULT_QR;
    } else if (!(cfg.qrTransferContent || '').trim()) {
      // Gợi ý mẫu khi bật chế độ tuỳ chỉnh; user vẫn xoá được để gõ lại.
      cfg.qrTransferContent = '{ma_lenh}';
    }
  }

  function applyConfig(next: BotConfig) {
    cfg = next;
    useBankDefaultQr = !(cfg.qrTransferContent || '').trim();
  }

  function logClass(line: string): string {
    if (line.includes('❌') || line.includes('Lỗi') || line.includes('thất bại')) return 'err';
    if (line.includes('✅') || line.includes('Đã gửi') || line.includes('hoạt động')) return 'ok';
    if (line.includes('⚠️') || line.includes('Yêu cầu dừng')) return 'warn';
    if (line.includes('[CHAT]')) return 'chat';
    if (line.includes('[INIT]') || line.includes('[UI]')) return 'muted';
    return '';
  }

  function pushLog(line: string) {
    logs = [...logs, line].slice(-500);
  }

  async function scrollLogs() {
    if (!autoScroll || !logBox) return;
    await tick();
    logBox.scrollTop = logBox.scrollHeight;
  }

  $: if (logs) scrollLogs();

  function toggleSection(section: Exclude<EditSection, null>) {
    editSection = editSection === section ? null : section;
  }

  onMount(async () => {
    try {
      applyConfig(await invoke<BotConfig>('get_bot_config'));
    } catch (e) {
      toastError('Không tải được cấu hình bot: ' + String(e));
    }

    try {
      const s = await invoke<string>('get_bot_status');
      if (s === 'running') status = 'running';
    } catch {
      /* ignore */
    }

    unsubs.push(
      await listen<string>('bot-log', (e) => pushLog(e.payload)),
      await listen<string>('bot-status', (e) => {
        const s = e.payload as BotStatus;
        status = s === 'stopping' ? 'stopping' : s === 'running' ? 'running' : 'idle';
      })
    );
  });

  onDestroy(() => {
    unsubs.forEach((u) => u());
  });

  function configForSave(): BotConfig {
    return {
      ...cfg,
      qrTransferContent: useBankDefaultQr
        ? BANK_DEFAULT_QR
        : (cfg.qrTransferContent || '').trim(),
    };
  }

  async function onSave() {
    saving = true;
    try {
      const toSave = configForSave();
      await invoke('save_bot_config', { cfg: toSave });
      applyConfig(toSave);
      toastSuccess('Đã lưu cấu hình bot');
    } catch (e) {
      toastError('Lưu thất bại: ' + String(e));
    } finally {
      saving = false;
    }
  }

  async function onStart() {
    try {
      const toSave = configForSave();
      await invoke('start_bot', { cfg: toSave });
      applyConfig(toSave);
      status = 'running';
      toastSuccess('Bot đã bắt đầu');
    } catch (e) {
      toastError(String(e));
    }
  }

  async function onStop() {
    try {
      await invoke('stop_bot');
      status = 'stopping';
    } catch (e) {
      toastError(String(e));
    }
  }

  async function copyLogs() {
    try {
      await navigator.clipboard.writeText(logs.join('\n'));
      toastSuccess('Đã copy nhật ký');
    } catch {
      toastError('Không copy được nhật ký');
    }
  }
</script>

<div class="bot" transition:fade>
  <!-- Primary control -->
  <section class="hero {status}">
    <div class="hero-left">
      <div class="live">
        <span class="pulse {status}"></span>
        <span class="live-label">{statusLabel(status)}</span>
      </div>
      <h2>Bot lệnh bán</h2>
      <p class="hero-desc">{statusHint(status)}</p>
    </div>

    <div class="hero-actions">
      {#if running || status === 'stopping'}
        <button
          type="button"
          class="primary stop"
          disabled={status !== 'running'}
          on:click={onStop}
        >
          Dừng bot
        </button>
      {:else}
        <button type="button" class="primary start" on:click={onStart}>
          Bắt đầu
        </button>
      {/if}
      <button
        type="button"
        class="secondary"
        disabled={busy || saving}
        on:click={onSave}
      >
        {saving ? 'Đang lưu…' : 'Lưu cấu hình'}
      </button>
    </div>
  </section>

  <div class="layout">
    <!-- Activity first -->
    <section class="panel log-panel">
      <header class="panel-head">
        <div>
          <h3>Hoạt động</h3>
          <p class="panel-sub">Tin gửi đi và sự kiện chat hiện ở đây</p>
        </div>
        <div class="tools">
          <label class="check">
            <input type="checkbox" bind:checked={autoScroll} />
            Tự cuộn
          </label>
          <button type="button" class="tool" on:click={copyLogs}>Copy</button>
          <button type="button" class="tool" on:click={() => (logs = [])}>Xóa</button>
        </div>
      </header>

      <div class="log-box" bind:this={logBox}>
        {#if logs.length === 0}
          <div class="empty">
            <div class="empty-title">Chưa có hoạt động</div>
            <div class="empty-sub">
              {running
                ? 'Đang chờ lệnh bán mới…'
                : 'Bấm Bắt đầu — bot sẽ ghi log tại đây khi có lệnh.'}
            </div>
          </div>
        {:else}
          {#each logs as line, i (i)}
            <div class="line {logClass(line)}">{line}</div>
          {/each}
        {/if}
      </div>
    </section>

    <!-- Compact settings -->
    <section class="panel settings-panel">
      <header class="panel-head">
        <div>
          <h3>Cấu hình</h3>
          <p class="panel-sub">Chỉ mở khi cần chỉnh tin hoặc tài khoản</p>
        </div>
      </header>

      {#if busy}
        <div class="lock">Đang chạy — dừng bot trước khi sửa.</div>
      {/if}

      <div class="accordion" class:disabled={busy}>
        <button
          type="button"
          class="acc-btn"
          class:open={editSection === 'messages'}
          disabled={busy}
          on:click={() => toggleSection('messages')}
        >
          <span class="acc-main">
            <span class="acc-title">Tin nhắn tự động</span>
            <span class="acc-preview">{greetingPreview}</span>
          </span>
          <span class="chev">{editSection === 'messages' ? '▴' : '▾'}</span>
        </button>
        {#if editSection === 'messages'}
          <div class="acc-body" transition:slide={{ duration: 160 }}>
            <p class="tip">
              Placeholder: <code>{'{ma_lenh}'}</code>
              <code>{'{so_tien}'}</code>
              <code>{'{ten_nguoi_mua}'}</code>
            </p>
            <label>
              Tin 1 — chào + quy định
              <span class="field-hint">Gửi ngay khi có lệnh bán chờ thanh toán (cùng ảnh QR)</span>
              <textarea rows="7" bind:value={cfg.greetingMessage}></textarea>
            </label>
            <label>
              Tin 3 — sau khi giao dịch xong
              <span class="field-hint">Chỉ gửi khi lệnh COMPLETED (đã hoàn tất)</span>
              <textarea rows="3" bind:value={cfg.instructionMessage}></textarea>
            </label>
          </div>
        {/if}

        <button
          type="button"
          class="acc-btn"
          class:open={editSection === 'qr'}
          disabled={busy}
          on:click={() => toggleSection('qr')}
        >
          <span class="acc-main">
            <span class="acc-title">Nội dung CK trong QR</span>
            <span class="acc-preview">{qrPreview}</span>
          </span>
          <span class="chev">{editSection === 'qr' ? '▴' : '▾'}</span>
        </button>
        {#if editSection === 'qr'}
          <div class="acc-body" transition:slide={{ duration: 160 }}>
            <p class="tip">
              Nội dung chuyển khoản nhúng vào ảnh VietQR. Để mặc định ngân hàng nếu không cần ghi chú riêng.
            </p>
            <label class="check block">
              <input
                type="checkbox"
                checked={useBankDefaultQr}
                on:change={onBankDefaultQrChange}
              />
              Dùng nội dung mặc định của ngân hàng
            </label>
            {#if !useBankDefaultQr}
              <label>
                Mẫu nội dung CK
                <span class="field-hint">
                  Placeholder: <code>{'{ma_lenh}'}</code>
                  <code>{'{so_tien}'}</code>
                  <code>{'{ten_nguoi_mua}'}</code>
                  — có thể xoá hết rồi gõ lại; để trống khi lưu = không ghi nội dung CK.
                </span>
                <div class="field-row">
                  <input
                    bind:value={cfg.qrTransferContent}
                    placeholder="{'{ma_lenh}'}"
                  />
                  <button
                    type="button"
                    class="tool"
                    on:click={() => (cfg.qrTransferContent = '{ma_lenh}')}
                  >
                    Mã lệnh
                  </button>
                </div>
              </label>
            {/if}
          </div>
        {/if}

        <button
          type="button"
          class="acc-btn"
          class:open={editSection === 'backup'}
          disabled={busy}
          on:click={() => toggleSection('backup')}
        >
          <span class="acc-main">
            <span class="acc-title">Tài khoản dự phòng</span>
            <span class="acc-preview">
              {cfg.bankName || cfg.accountNo
                ? [cfg.bankName, cfg.accountNo].filter(Boolean).join(' · ')
                : 'Để trống — lấy từ lệnh Binance'}
            </span>
          </span>
          <span class="chev">{editSection === 'backup' ? '▴' : '▾'}</span>
        </button>
        {#if editSection === 'backup'}
          <div class="acc-body" transition:slide={{ duration: 160 }}>
            <p class="tip">Chỉ dùng khi lệnh thiếu payMethods.</p>
            <div class="grid">
              <label>
                Ngân hàng
                <input bind:value={cfg.bankName} placeholder="MB, BIDV…" />
              </label>
              <label>
                Số tài khoản
                <input bind:value={cfg.accountNo} />
              </label>
            </div>
            <label>
              Chủ tài khoản
              <input bind:value={cfg.accountName} />
            </label>
          </div>
        {/if}

        <button
          type="button"
          class="acc-btn last"
          class:open={editSection === 'timing'}
          disabled={busy}
          on:click={() => toggleSection('timing')}
        >
          <span class="acc-main">
            <span class="acc-title">Chu kỳ quét</span>
            <span class="acc-preview">
              mỗi {cfg.pollIntervalSecs}s · lệnh ≤ {cfg.orderMaxAgeMinutes} phút
            </span>
          </span>
          <span class="chev">{editSection === 'timing' ? '▴' : '▾'}</span>
        </button>
        {#if editSection === 'timing'}
          <div class="acc-body last" transition:slide={{ duration: 160 }}>
            <div class="grid">
              <label>
                Chu kỳ (giây)
                <input type="number" min="5" bind:value={cfg.pollIntervalSecs} />
              </label>
              <label>
                Tuổi lệnh tối đa (phút)
                <input type="number" min="5" bind:value={cfg.orderMaxAgeMinutes} />
              </label>
            </div>
          </div>
        {/if}
      </div>
    </section>
  </div>
</div>

<style>
  .bot {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* ── Hero ── */
  .hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    flex-wrap: wrap;
    padding: 22px 24px;
    border-radius: 12px;
    background: linear-gradient(135deg, rgba(31, 41, 55, 0.9) 0%, rgba(17, 24, 39, 0.75) 100%);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-left: 4px solid #6b7280;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
  }
  .hero.running {
    border-left-color: #22c55e;
  }
  .hero.stopping {
    border-left-color: #f59e0b;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .pulse {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #6b7280;
    box-shadow: 0 0 0 3px rgba(107, 114, 128, 0.25);
  }
  .pulse.running {
    background: #22c55e;
    box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.3);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .pulse.stopping {
    background: #f59e0b;
    box-shadow: 0 0 0 3px rgba(245, 158, 11, 0.3);
  }

  @keyframes pulse {
    0%,
    100% {
      box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.25);
    }
    50% {
      box-shadow: 0 0 0 6px rgba(34, 197, 94, 0.12);
    }
  }

  .live-label {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #9ca3af;
  }
  .hero.running .live-label {
    color: #4ade80;
  }
  .hero.stopping .live-label {
    color: #fbbf24;
  }

  .hero h2 {
    margin: 0;
    font-size: 22px;
    line-height: 1.2;
  }

  .hero-desc {
    margin: 6px 0 0;
    color: #9ca3af;
    font-size: 13px;
    max-width: 420px;
    line-height: 1.45;
  }

  .hero-actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  /* Override global button lift/glow for this panel */
  .bot :global(button) {
    box-shadow: none !important;
  }
  .bot :global(button:hover:not(:disabled)) {
    transform: none !important;
  }

  .primary {
    min-width: 128px;
    height: 40px;
    padding: 0 20px !important;
    font-size: 13px !important;
    border-radius: 8px !important;
    border: 1px solid transparent !important;
  }
  .primary.start {
    background: #059669 !important;
    border-color: #10b981 !important;
    color: #fff !important;
  }
  .primary.start:hover:not(:disabled) {
    background: #10b981 !important;
  }
  .primary.stop {
    background: #dc2626 !important;
    border-color: #ef4444 !important;
    color: #fff !important;
  }
  .primary.stop:hover:not(:disabled) {
    background: #ef4444 !important;
  }

  .secondary {
    height: 40px;
    padding: 0 16px !important;
    font-size: 13px !important;
    border-radius: 8px !important;
    background: rgba(37, 99, 235, 0.12) !important;
    border: 1px solid rgba(37, 99, 235, 0.4) !important;
    color: #93c5fd !important;
  }
  .secondary:hover:not(:disabled) {
    background: rgba(37, 99, 235, 0.22) !important;
    border-color: rgba(37, 99, 235, 0.6) !important;
    color: #bfdbfe !important;
  }

  /* ── Layout ── */
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(280px, 0.9fr);
    gap: 16px;
    align-items: start;
  }

  @media (max-width: 960px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  .panel {
    background: rgba(31, 41, 55, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
    overflow: hidden;
  }

  .panel-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    padding: 16px 18px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .panel-head h3 {
    margin: 0;
    font-size: 15px;
  }

  .panel-sub {
    margin: 4px 0 0;
    font-size: 12px;
    color: #6b7280;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 12px;
    color: #9ca3af;
    cursor: pointer;
    white-space: nowrap;
  }

  .check.block {
    white-space: normal;
    margin-bottom: 12px;
    color: #d1d5db;
  }

  .field-hint {
    display: block;
    margin-top: 4px;
    font-size: 11px;
    font-weight: 400;
    color: #6b7280;
    line-height: 1.4;
  }

  .tool {
    height: 30px;
    padding: 0 12px !important;
    font-size: 12px !important;
    font-weight: 600 !important;
    border-radius: 6px !important;
    background: rgba(37, 99, 235, 0.1) !important;
    border: 1px solid rgba(37, 99, 235, 0.3) !important;
    color: #60a5fa !important;
  }
  .tool:hover:not(:disabled) {
    background: rgba(37, 99, 235, 0.2) !important;
    border-color: rgba(37, 99, 235, 0.5) !important;
    color: #93c5fd !important;
  }

  /* ── Log ── */
  .log-box {
    height: min(52vh, 420px);
    overflow: auto;
    padding: 14px 16px;
    background: rgba(15, 23, 42, 0.45);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.55;
    scrollbar-width: thin;
    scrollbar-color: #374151 transparent;
  }

  .log-box::-webkit-scrollbar {
    width: 8px;
  }
  .log-box::-webkit-scrollbar-track {
    background: transparent;
  }
  .log-box::-webkit-scrollbar-thumb {
    background: #374151;
    border-radius: 8px;
    border: 2px solid transparent;
    background-clip: padding-box;
  }
  .log-box::-webkit-scrollbar-thumb:hover {
    background: #4b5563;
    background-clip: padding-box;
  }

  .empty {
    height: 100%;
    min-height: 180px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 24px;
  }
  .empty-title {
    color: #d1d5db;
    font-weight: 600;
    font-size: 14px;
    font-family: inherit;
  }
  .empty-sub {
    margin-top: 6px;
    color: #6b7280;
    font-size: 12px;
    max-width: 280px;
    font-family: inherit;
    line-height: 1.5;
  }

  .line {
    white-space: pre-wrap;
    word-break: break-word;
    color: #d1d5db;
    padding: 2px 0;
  }
  .line.err {
    color: #fca5a5;
  }
  .line.ok {
    color: #4ade80;
  }
  .line.warn {
    color: #fbbf24;
  }
  .line.chat {
    color: #67e8f9;
  }
  .line.muted {
    color: #9ca3af;
  }

  /* ── Settings accordion ── */
  .lock {
    margin: 12px 16px 0;
    padding: 10px 12px;
    font-size: 12px;
    color: #fbbf24;
    background: rgba(245, 158, 11, 0.1);
    border-left: 3px solid #f59e0b;
    border-radius: 6px;
  }

  .accordion {
    padding: 8px;
  }
  .accordion.disabled {
    opacity: 0.72;
  }

  .acc-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    text-align: left;
    background: transparent !important;
    box-shadow: none !important;
    border: 1px solid transparent !important;
    border-radius: 8px !important;
    padding: 12px 12px !important;
    color: #e5e7eb !important;
  }
  .acc-btn:hover:not(:disabled) {
    transform: none !important;
    background: rgba(255, 255, 255, 0.04) !important;
  }
  .acc-btn.open {
    background: rgba(37, 99, 235, 0.1) !important;
    border-color: rgba(37, 99, 235, 0.25) !important;
  }
  .acc-btn.last {
    margin-bottom: 0;
  }

  .acc-main {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .acc-title {
    font-size: 13px;
    font-weight: 600;
  }
  .acc-preview {
    font-size: 12px;
    color: #6b7280;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }
  .chev {
    color: #6b7280;
    font-size: 12px;
    flex-shrink: 0;
  }

  .acc-body {
    padding: 4px 12px 16px;
  }
  .acc-body.last {
    padding-bottom: 12px;
  }

  .tip {
    margin: 0 0 12px;
    font-size: 12px;
    color: #9ca3af;
    line-height: 1.5;
  }

  code {
    background: rgba(255, 255, 255, 0.06);
    color: #fbbf24;
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 11px;
    margin-right: 4px;
  }

  label {
    display: block;
    margin-bottom: 12px;
    color: #9ca3af;
    font-size: 12px;
    font-weight: 500;
  }

  textarea,
  input:not([type='checkbox']) {
    display: block;
    width: 100%;
    margin-top: 6px;
    background: #111827;
    color: #e5e7eb;
    border: 1px solid #374151;
    padding: 10px 12px;
    border-radius: 8px;
    font-size: 13px;
    font-family: inherit;
    box-sizing: border-box;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  textarea {
    resize: vertical;
    line-height: 1.45;
    min-height: 64px;
    scrollbar-width: thin;
    scrollbar-color: #374151 transparent;
  }

  textarea::-webkit-scrollbar {
    width: 8px;
  }
  textarea::-webkit-scrollbar-track {
    background: transparent;
  }
  textarea::-webkit-scrollbar-thumb {
    background: #374151;
    border-radius: 8px;
  }
  textarea::-webkit-scrollbar-thumb:hover {
    background: #4b5563;
  }

  textarea:focus,
  input:not([type='checkbox']):focus {
    outline: none;
    border-color: #2563eb;
    box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field-row {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 6px;
  }
  .field-row input {
    margin-top: 0;
    flex: 1;
  }

  @media (max-width: 640px) {
    .hero-actions {
      width: 100%;
    }
    .hero-actions .primary,
    .hero-actions .secondary {
      flex: 1;
    }
    .grid {
      grid-template-columns: 1fr;
    }
    .log-box {
      height: 280px;
    }
  }
</style>
