import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// This checks the required scalar fields, not arbitrary optional YAML metadata.
function scalar(value, lines, index) {
  value = value.trim();
  if (/^[|>][-+]?$/.test(value)) {
    const block = [];
    for (let i = index + 1; i < lines.length && /^\s/.test(lines[i]); i++) block.push(lines[i].trim());
    return block.join(value.startsWith('>') ? ' ' : '\n').trim();
  }
  if (value.startsWith('"')) {
    const quoted = /^("(?:[^"\\]|\\.)*")\s*(?:#.*)?$/.exec(value);
    if (!quoted) throw Error('invalid quoted scalar');
    return JSON.parse(quoted[1]);
  }
  if (value.startsWith("'")) {
    const quoted = /^('(?:[^']|'')*')\s*(?:#.*)?$/.exec(value);
    if (!quoted) throw Error('invalid quoted scalar');
    return quoted[1].slice(1, -1).replaceAll("''", "'");
  }
  value = value.replace(/\s+#.*$/, '').trim();
  if (/^(?:null|~|true|false|[-+]?\d+(?:\.\d+)?)$/i.test(value) || /^[\[\]{}]|:\s/.test(value)) throw Error('expected a string scalar');
  return value;
}

function inspect(file, directory) {
  const text = fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '').replaceAll('\r\n', '\n');
  const lines = text.split('\n');
  if (lines[0] !== '---') throw Error('missing opening frontmatter delimiter');
  const end = lines.indexOf('---', 1);
  if (end < 0) throw Error('missing closing frontmatter delimiter');
  const frontmatter = lines.slice(1, end);
  const fields = {};
  for (let index = 0; index < frontmatter.length; index++) {
    const match = /^(name|description):\s*(.*)$/.exec(frontmatter[index]);
    if (!match) continue;
    const [, key, value] = match;
    if (Object.hasOwn(fields, key)) throw Error(`duplicate ${key} field`);
    fields[key] = scalar(value, frontmatter, index);
  }
  for (const key of ['name', 'description']) {
    if (typeof fields[key] !== 'string' || !fields[key].trim()) throw Error(`missing or empty ${key} string in frontmatter`);
  }
  if (!/^[a-z0-9_][a-z0-9_-]{0,63}$/.test(fields.name)) throw Error('invalid skill name');
  if (fields.name !== directory) throw Error(`name ${fields.name} does not match directory ${directory}`);
  return /\b(?:workflow|step \d+|phase \d+|dispatch)\b/i.test(fields.description);
}

let root = fileURLToPath(new URL('../skills/', import.meta.url));
let dryRun = false;
try {
  for (let i = 2; i < process.argv.length; i++) {
    if (process.argv[i] === '--dry-run') dryRun = true;
    else if (process.argv[i] === '--root' && process.argv[i + 1]) root = path.resolve(process.argv[++i]);
    else throw Error(`unknown or incomplete argument: ${process.argv[i]}`);
  }
  const directories = fs.readdirSync(root, { withFileTypes: true }).filter(entry => entry.isDirectory()).sort((a, b) => a.name.localeCompare(b.name));
  if (!directories.length) throw Error(`no skill directories found in ${root}`);
  let errors = 0, warnings = 0;
  for (const directory of directories) {
    const file = path.join(root, directory.name, 'SKILL.md');
    try {
      if (!fs.existsSync(file)) throw Error('missing SKILL.md');
      if (inspect(file, directory.name)) {
        warnings++;
        console.log(`CSO WARNING: ${file}: description may summarize workflow instead of trigger conditions`);
      }
      console.log(`OK: ${file}`);
    } catch (error) {
      errors++;
      console.error(`FAIL: ${file}: ${error.message}`);
    }
  }
  console.log(`${directories.length} skills checked; ${errors} errors; ${warnings} advisory warnings.${dryRun ? ' Dry-run: no files modified.' : ''}`);
  if (errors) process.exitCode = 1;
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
}
