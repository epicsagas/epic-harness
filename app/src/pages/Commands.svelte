<script lang="ts">
  interface Command {
    cmd: string;
    desc: string;
    ring: number;
  }

  const commands: Command[] = [
    { cmd: 'discover', desc: 'Explore and define the problem before specifying a solution', ring: 1 },
    { cmd: 'spec', desc: 'Define requirements before coding', ring: 1 },
    { cmd: 'go', desc: 'Build with auto-plan + TDD', ring: 1 },
    { cmd: 'check', desc: 'Review + security audit + tests', ring: 1 },
    { cmd: 'ship', desc: 'Create PR, verify CI, merge', ring: 1 },
    { cmd: 'evolve', desc: 'Inspect or trigger skill evolution', ring: 3 },
    { cmd: 'team', desc: 'Generate project-specific agent team', ring: 1 },
    { cmd: 'orbit', desc: 'Autonomous spec→ship pipeline', ring: 1 },
    { cmd: 'git-cc', desc: 'Conventional commit with auto type selection', ring: 0 },
    { cmd: 'git', desc: 'Cross-repo git operations (sync/bump/tags)', ring: 0 },
  ];

  let copied = $state<string | null>(null);

  async function copyCmd(cmd: string) {
    try {
      await navigator.clipboard.writeText('/' + cmd);
      copied = cmd;
      setTimeout(() => {
        copied = null;
      }, 1500);
    } catch {
      // Clipboard API unavailable (non-secure context)
      copied = null;
    }
  }

  function ringClass(ring: number): string {
    if (ring === 0) return 'pill orange';
    if (ring === 3) return 'pill purple';
    return 'pill info';
  }
</script>

<div class="screen-header">
  <h2>Commands <span class="subtitle-tag">Ring 1</span></h2>
  <p>10 user-invoked slash commands — click any card to copy</p>
</div>

<div class="grid-2">
  {#each commands as { cmd, desc, ring }}
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
        {#if copied === cmd}
          <span style="font-size:11px;color:var(--success);font-family:var(--font-mono);font-weight:600;">Copied!</span>
        {/if}
      </div>
      <div class="cmd-desc">{desc}</div>
      <div class="cmd-tags" style="margin-top:8px;">
        <span class={ringClass(ring)}>Ring {ring}</span>
      </div>
    </div>
  {/each}
</div>
