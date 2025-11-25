<!-- Skeleton Loading Component -->
<script lang="ts">
  export let rows: number = 5;
</script>

<div class="skeleton-table">
  <div class="skeleton-header">
    {#each Array(6) as _}
      <div class="skeleton-cell"></div>
    {/each}
  </div>
  
  {#each Array(rows) as _, i}
    <div class="skeleton-row" style="animation-delay: {i * 50}ms;">
      {#each Array(6) as _}
        <div class="skeleton-cell"></div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .skeleton-table {
    background: #1f2937;
    border-radius: 8px;
    border: 1px solid #374151;
    overflow: hidden;
  }
  
  .skeleton-header,
  .skeleton-row {
    display: grid;
    grid-template-columns: 1.5fr 2fr 1fr 1.5fr 1.2fr 1fr;
    gap: 1px;
    background: #374151;
  }
  
  .skeleton-header {
    padding: 12px;
  }
  
  .skeleton-row {
    animation: fadeIn 0.5s ease-out forwards;
    opacity: 0;
  }
  
  @keyframes fadeIn {
    to {
      opacity: 1;
    }
  }
  
  .skeleton-cell {
    background: linear-gradient(
      90deg,
      #1f2937 0%,
      #374151 20%,
      #1f2937 40%,
      #1f2937 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
    height: 44px;
    border-radius: 4px;
  }
  
  .skeleton-header .skeleton-cell {
    height: 32px;
  }
  
  @keyframes shimmer {
    0% {
      background-position: -200% 0;
    }
    100% {
      background-position: 200% 0;
    }
  }
</style>
