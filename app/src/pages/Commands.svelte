<script lang="ts">
  import { tStore } from '$lib/i18n.js';

  type CommandKey =
    | 'cmdOrbitDesc' | 'cmdEvolveDesc' | 'cmdTeamDesc';

  interface Command {
    cmd: string;
    descKey: CommandKey;
    ring: number;
  }

  const commands: Command[] = [
    { cmd: 'orbit',    descKey: 'cmdOrbitDesc',    ring: 1 },
    { cmd: 'evolve',   descKey: 'cmdEvolveDesc',   ring: 3 },
    { cmd: 'team',     descKey: 'cmdTeamDesc',     ring: 1 },
  ];

  let copiedCmd = $state<string | null>(null);

  async function copyCmd(cmd: string) {
    try {
      await navigator.clipboard.writeText('/' + cmd);
      copiedCmd = cmd;
      setTimeout(() => { copiedCmd = null; }, 1500);
    } catch {
      copiedCmd = null;
    }
  }

  function ringClass(ring: number): string {
    if (ring === 0) return 'pill orange';
    if (ring === 3) return 'pill purple';
    return 'pill info';
  }
</script>

<div class="screen-header">
  <h2>{$tStore('pageCommands')} <span class="subtitle-tag">Ring 1</span></h2>
  <p>{$tStore('pageCommandsDesc3')}</p>
</div>

<div class="grid-2">
  {#each commands as { cmd, descKey, ring }}
    <div
      class="cmd-card"
      role="button"
      tabindex="0"
      style="cursor:pointer;transition:background var(--transition),box-shadow var(--transition);"
      onclick={() => copyCmd(cmd)}
      onkeydown={(e) => e.key === 'Enter' && copyCmd(cmd)}
    >
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px;">
        <div class="cmd-name">/{cmd}</div>
        {#if copiedCmd === cmd}
          <span style="font-size:11px;color:var(--success);font-family:var(--font-mono);font-weight:600;">{$tStore('copied')}</span>
        {/if}
      </div>
      <div class="cmd-desc">{$tStore(descKey)}</div>
      <div class="cmd-tags" style="margin-top:8px;">
        <span class={ringClass(ring)}>Ring {ring}</span>
      </div>
    </div>
  {/each}
</div>
