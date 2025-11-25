<!-- Pagination Component -->
<script lang="ts">
  export let currentPage: number = 1;
  export let totalPages: number = 1;
  export let onPageChange: (page: number) => void;
  
  $: pages = generatePageNumbers(currentPage, totalPages);
  
  function generatePageNumbers(current: number, total: number): (number | string)[] {
    if (total <= 7) {
      return Array.from({ length: total }, (_, i) => i + 1);
    }
    
    const pages: (number | string)[] = [];
    
    // Always show first page
    pages.push(1);
    
    if (current > 3) {
      pages.push('...');
    }
    
    // Show current page and neighbors
    for (let i = Math.max(2, current - 1); i <= Math.min(total - 1, current + 1); i++) {
      pages.push(i);
    }
    
    if (current < total - 2) {
      pages.push('...');
    }
    
    // Always show last page
    if (total > 1) {
      pages.push(total);
    }
    
    return pages;
  }
  
  function goToPage(page: number) {
    if (page >= 1 && page <= totalPages && page !== currentPage) {
      onPageChange(page);
    }
  }
  
  function nextPage() {
    if (currentPage < totalPages) {
      goToPage(currentPage + 1);
    }
  }
  
  function prevPage() {
    if (currentPage > 1) {
      goToPage(currentPage - 1);
    }
  }
</script>

{#if totalPages > 1}
  <div class="pagination">
    <button 
      class="pagination-btn"
      disabled={currentPage === 1}
      on:click={prevPage}
    >
      ‹ Trước
    </button>
    
    {#each pages as page}
      {#if page === '...'}
        <span class="pagination-ellipsis">...</span>
      {:else}
        <button 
          class="pagination-btn page-number"
          class:active={page === currentPage}
          on:click={() => goToPage(typeof page === 'number' ? page : currentPage)}
        >
          {page}
        </button>
      {/if}
    {/each}
    
    <button 
      class="pagination-btn"
      disabled={currentPage === totalPages}
      on:click={nextPage}
    >
      Sau ›
    </button>
  </div>
{/if}

<style>
  .pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin: 24px 0;
    flex-wrap: wrap;
  }
  
  .pagination-btn {
    background: #1e293b;
    color: #94a3b8;
    border: 1px solid #334155;
    padding: 8px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s ease;
    min-width: 40px;
  }
  
  .pagination-btn:hover:not(:disabled) {
    background: #334155;
    color: #e2e8f0;
    border-color: #475569;
    transform: translateY(-1px);
  }
  
  .pagination-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  
  .pagination-btn.page-number {
    min-width: 36px;
    padding: 8px 12px;
  }
  
  .pagination-btn.active {
    background: #2563eb;
    color: white;
    border-color: #2563eb;
    font-weight: 600;
  }
  
  .pagination-ellipsis {
    color: #64748b;
    padding: 0 4px;
    user-select: none;
  }
</style>
