<script lang="ts">
  import { tStore } from '$lib/i18n.js';

  type CommandKey =
    | 'cmdDiscoverDesc' | 'cmdSpecDesc' | 'cmdGoDesc' | 'cmdCheckDesc' | 'cmdShipDesc'
    | 'cmdEvolveDesc' | 'cmdTeamDesc' | 'cmdOrbitDesc' | 'cmdGitCcDesc' | 'cmdGitDesc';

  interface Command {
    cmd: string;
    descKey: CommandKey;
    ring: number;
  }

  const commands: Command[] = [
    { cmd: 'discover', descKey: 'cmdDiscoverDesc', ring: 1 },
    { cmd: 'spec',     descKey: 'cmdSpecDesc',     ring: 1 },
    { cmd: 'go',       descKey: 'cmdGoDesc',        ring: 1 },
    { cmd: 'check',    descKey: 'cmdCheckDesc',     ring: 1 },
    { cmd: 'ship',     descKey: 'cmdShipDesc',      ring: 1 },
    { cmd: 'evolve',   descKey: 'cmdEvolveDesc',    ring: 3 },
    { cmd: 'team',     descKey: 'cmdTeamDesc',      ring: 1 },
    { cmd: 'orbit',    descKey: 'cmdOrbitDesc',     ring: 1 },
    { cmd: 'git-cc',   descKey: 'cmdGitCcDesc',     ring: 0 },
    { cmd: 'git',      descKey: 'cmdGitDesc',       ring: 0 },
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
  <p>{$tStore('pageCommandsDesc')}</p>
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
