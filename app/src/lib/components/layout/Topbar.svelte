<script lang="ts">
  interface Props {
    currentLabel: string;
  }

  let { currentLabel }: Props = $props();
</script>

<div class="topbar">
  <div class="topbar-left">
    <div class="breadcrumb">
      epic-harness <span>/</span> <span>{currentLabel}</span>
    </div>
  </div>
  <div class="topbar-right">
    <div class="search-bar">
      <span class="search-icon">&#8981;</span>
      <input type="text" placeholder="Search..." />
    </div>
    <button class="icon-btn" title="Notifications">&#128276;</button>
  </div>
</div>
