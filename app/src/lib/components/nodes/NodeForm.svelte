<script lang="ts">
  import { addNode } from '$lib/stores/graph';

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let type = $state('concept');
  let title = $state('');
  let body = $state('');
  let tags = $state('');
  let project = $state('');
  let submitting = $state(false);

  const types = ['decision', 'resolution', 'concept', 'project', 'pattern', 'error', 'session', 'instinct', 'psychographic'];

  async function handleSubmit() {
    if (!title.trim()) return;
    submitting = true;
    try {
      await addNode({
        type,
        title: title.trim(),
        body: body.trim(),
        tags: tags.trim() ? tags.split(',').map((t) => t.trim()).filter(Boolean) : undefined,
        projects: project.trim() ? [project.trim()] : undefined,
      });
      onclose();
    } catch (e) {
      console.error('Failed to create node:', e);
    } finally {
      submitting = false;
    }
  }
</script>

<div class="space-y-4">
  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Type</label>
    <select bind:value={type} class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm">
      {#each types as t}
        <option value={t}>{t}</option>
      {/each}
    </select>
  </div>

  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Title</label>
    <input bind:value={title} placeholder="Node title..." class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm" />
  </div>

  <div>
    <label class="block text-xs font-bold text-on-surface-variant mb-1">Body</label>
    <textarea bind:value={body} rows={6} placeholder="Markdown content..." class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm font-mono"></textarea>
  </div>

  <div class="grid grid-cols-2 gap-3">
    <div>
      <label class="block text-xs font-bold text-on-surface-variant mb-1">Tags</label>
      <input bind:value={tags} placeholder="tag1, tag2" class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm" />
    </div>
    <div>
      <label class="block text-xs font-bold text-on-surface-variant mb-1">Project</label>
      <input bind:value={project} placeholder="project-slug" class="w-full bg-surface-container border border-outline-variant rounded-lg px-3 py-2 text-on-surface text-sm" />
    </div>
  </div>

  <div class="flex gap-2 pt-2">
    <button onclick={onclose} class="flex-1 px-4 py-2 rounded-lg border border-outline-variant text-on-surface text-sm hover:bg-surface-variant/30 transition-colors">Cancel</button>
    <button onclick={handleSubmit} disabled={submitting || !title.trim()} class="flex-1 px-4 py-2 rounded-lg bg-primary text-on-primary text-sm font-bold hover:opacity-90 transition-colors disabled:opacity-50">
      {submitting ? 'Creating...' : 'Create Node'}
    </button>
  </div>
</div>
