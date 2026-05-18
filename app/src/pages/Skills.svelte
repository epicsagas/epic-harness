<script lang="ts">
  interface Skill {
    name: string;
    desc: string;
    special?: boolean;
  }

  const skills: Skill[] = [
    { name: '_dispatch', desc: 'Auto-routes tasks to the right skill based on context', special: true },
    { name: 'tdd', desc: 'Test-first development cycle' },
    { name: 'debug', desc: 'Systematic root cause analysis' },
    { name: 'secure', desc: 'Security checklist for auth/db/api code' },
    { name: 'verify', desc: 'Build + test + lint before marking done' },
    { name: 'simplify', desc: 'Triggered on files > 200 lines' },
    { name: 'perf', desc: 'Performance analysis for DB/API code' },
    { name: 'review', desc: 'Code quality and logic review' },
    { name: 'refactor', desc: 'Safe structural improvement' },
    { name: 'migrate', desc: 'Database and schema migration safety' },
    { name: 'api-design', desc: 'REST/GraphQL API design patterns' },
    { name: 'doc', desc: 'Documentation generation' },
    { name: 'test-gen', desc: 'Auto test generation for uncovered code' },
    { name: 'ci', desc: 'CI/CD pipeline configuration' },
    { name: 'deploy', desc: 'Safe deployment checklist' },
  ];

  let copied = $state<string | null>(null);

  async function copySkill(name: string) {
    try {
      await navigator.clipboard.writeText('/' + name);
      copied = name;
      setTimeout(() => {
        copied = null;
      }, 1500);
    } catch {
      // Clipboard API unavailable (non-secure context)
      copied = null;
    }
  }
</script>

<div class="screen-header">
  <h2>Auto Skills <span class="subtitle-tag">Ring 2</span></h2>
  <p>15 context-triggered skills + _dispatch core router &middot; click any card to copy</p>
</div>

<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
  {#each skills as skill}
    <div
      class="skill-card"
      role="button"
      tabindex="0"
      style={skill.special
        ? 'cursor:pointer;border:1px solid var(--accent);background:var(--surface-raised);transition:box-shadow var(--transition);'
        : 'cursor:pointer;transition:background var(--transition),box-shadow var(--transition);'}
      onclick={() => copySkill(skill.name)}
      onkeydown={(e) => e.key === 'Enter' && copySkill(skill.name)}
    >
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px;">
        <div class="skill-name" style={skill.special ? 'color:var(--accent);' : ''}>
          {skill.name}
          {#if skill.special}
            <span class="pill info" style="margin-left:6px;font-size:10px;">core router</span>
          {/if}
        </div>
        {#if copied === skill.name}
          <span style="font-size:11px;color:var(--success);font-family:var(--font-mono);font-weight:600;">Copied!</span>
        {/if}
      </div>
      <div class="skill-desc">{skill.desc}</div>
    </div>
  {/each}
</div>
