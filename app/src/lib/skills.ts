/** Build the dashboard catalog from the same files the plugin ships. */
const sources = import.meta.glob('../../../skills/*/SKILL.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const pipelines = new Set(['spec', 'go', 'audit', 'ship', 'discover', 'orbit', 'team', 'evolve', 'eval']);

export const skillCatalog = Object.entries(sources).map(([file, content]) => {
  const frontmatter = content.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1] ?? '';
  const name = frontmatter.match(/^name:\s*["']?([^"'\r\n]+)["']?\s*$/m)?.[1]?.trim() ?? file.split('/').at(-2)!;
  const rawDescription = frontmatter.match(/^description:\s*(.+)$/m)?.[1]?.trim() ?? name;
  const description = rawDescription.replace(/^["']|["']$/g, '');
  return { name, description, special: name === '_dispatch', pipeline: pipelines.has(name) };
}).sort((a, b) => Number(b.special) - Number(a.special) || a.name.localeCompare(b.name));
