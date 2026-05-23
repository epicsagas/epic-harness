<script lang="ts">
  import { tStore } from '$lib/i18n.js';

  type SkillDescKey =
    | 'skillDispatchDesc' | 'skillTddDesc' | 'skillDebugDesc' | 'skillSecureDesc'
    | 'skillVerifyDesc' | 'skillSimplifyDesc' | 'skillPerfDesc' | 'skillReviewDesc'
    | 'skillRefactorDesc' | 'skillMigrateDesc' | 'skillApiDesignDesc' | 'skillDocDesc'
    | 'skillTestGenDesc' | 'skillCiDesc' | 'skillDeployDesc'
    | 'skillSpecDesc' | 'skillGoDesc' | 'skillCheckDesc' | 'skillShipDesc';

  interface Skill {
    name: string;
    descKey: SkillDescKey;
    special?: boolean;
    pipeline?: boolean;
  }

  const skills: Skill[] = [
    { name: '_dispatch', descKey: 'skillDispatchDesc', special: true },
    { name: 'spec',      descKey: 'skillSpecDesc',     pipeline: true },
    { name: 'go',        descKey: 'skillGoDesc',       pipeline: true },
    { name: 'check',     descKey: 'skillCheckDesc',    pipeline: true },
    { name: 'ship',      descKey: 'skillShipDesc',     pipeline: true },
    { name: 'tdd',       descKey: 'skillTddDesc' },
    { name: 'debug',     descKey: 'skillDebugDesc' },
    { name: 'secure',    descKey: 'skillSecureDesc' },
    { name: 'verify',    descKey: 'skillVerifyDesc' },
    { name: 'simplify',  descKey: 'skillSimplifyDesc' },
    { name: 'perf',      descKey: 'skillPerfDesc' },
    { name: 'review',    descKey: 'skillReviewDesc' },
    { name: 'refactor',  descKey: 'skillRefactorDesc' },
    { name: 'migrate',   descKey: 'skillMigrateDesc' },
    { name: 'api-design',descKey: 'skillApiDesignDesc' },
    { name: 'doc',       descKey: 'skillDocDesc' },
    { name: 'test-gen',  descKey: 'skillTestGenDesc' },
    { name: 'ci',        descKey: 'skillCiDesc' },
    { name: 'deploy',    descKey: 'skillDeployDesc' },
  ];

  let copiedSkill = $state<string | null>(null);

  async function copySkill(name: string) {
    try {
      await navigator.clipboard.writeText('/' + name);
      copiedSkill = name;
      setTimeout(() => { copiedSkill = null; }, 1500);
    } catch {
      copiedSkill = null;
    }
  }

  function cardStyle(skill: Skill): string {
    if (skill.special) return 'cursor:pointer;border:1px solid var(--accent);background:var(--surface-raised);transition:box-shadow var(--transition);';
    if (skill.pipeline) return 'cursor:pointer;border:1px solid var(--teal);background:var(--surface-raised);transition:box-shadow var(--transition);';
    return 'cursor:pointer;transition:background var(--transition),box-shadow var(--transition);';
  }

  function nameStyle(skill: Skill): string {
    if (skill.special) return 'color:var(--accent);';
    if (skill.pipeline) return 'color:var(--teal);';
    return '';
  }
</script>

<div class="screen-header">
  <h2>{$tStore('pageSkills')} <span class="subtitle-tag">Ring 2</span></h2>
  <p>{$tStore('pageSkillsDesc19')}</p>
</div>

<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
  {#each skills as skill}
    <div
      class="skill-card"
      role="button"
      tabindex="0"
      style={cardStyle(skill)}
      onclick={() => copySkill(skill.name)}
      onkeydown={(e) => e.key === 'Enter' && copySkill(skill.name)}
    >
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:4px;">
        <div class="skill-name" style={nameStyle(skill)}>
          {skill.name}
          {#if skill.special}
            <span class="pill info" style="margin-left:6px;font-size:10px;">core router</span>
          {/if}
          {#if skill.pipeline}
            <span class="pill info" style="margin-left:6px;font-size:10px;background:var(--teal-soft);color:var(--teal);">pipeline</span>
          {/if}
        </div>
        {#if copiedSkill === skill.name}
          <span style="font-size:11px;color:var(--success);font-family:var(--font-mono);font-weight:600;">{$tStore('copied')}</span>
        {/if}
      </div>
      <div class="skill-desc">{$tStore(skill.descKey)}</div>
    </div>
  {/each}
</div>
