# Installing epic-harness for Gemini CLI

## Prerequisites

Ensure `epic-harness` binary is in your PATH:

```bash
# Verify installation
epic-harness --version

# If not found, install from the project root:
cargo install --path .
# or copy the pre-built binary to a directory in your PATH
```

## Step 1: Install settings.json

Copy the hook configuration to your project or global Gemini CLI settings.

**Project-level** (recommended — affects only this project):
```bash
mkdir -p .gemini
cp integrations/gemini/settings.json .gemini/settings.json
```

**Global** (affects all Gemini CLI sessions):
```bash
mkdir -p ~/.gemini
cp integrations/gemini/settings.json ~/.gemini/settings.json
```

If you already have a `.gemini/settings.json`, merge the `hooks` section manually.

## Step 2: Append GEMINI.md snippet

Add the epic-harness section to your project's `GEMINI.md`:

```bash
cat integrations/gemini/GEMINI.md >> GEMINI.md
```

If no `GEMINI.md` exists yet:
```bash
cp integrations/gemini/GEMINI.md GEMINI.md
```

## Step 3: Install commands

Copy the command definitions so Gemini CLI can load them:

```bash
mkdir -p .gemini/commands
cp integrations/gemini/commands/*.toml .gemini/commands/
```

## Step 4: Verify

Start a new Gemini CLI session. You should see output like:
```
[harness] Session started — loading memory and skills
```

If you see `[harness] epic-harness not found`, ensure the binary is in your PATH and retry.

## File Reference

| File | Purpose |
|------|---------|
| `settings.json` | Hook event bindings (BeforeAgent, AfterAgent, BeforeModel, AfterModel) |
| `GEMINI.md` | Snippet to append to project GEMINI.md |
| `commands/*.toml` | /spec, /go, /check, /ship, /evolve, /team command definitions |
| `skills/*/SKILL.md` | Auto-triggered skill definitions |
| `agents/*.md` | Builder, reviewer, auditor, planner agent definitions |

## Hook Event Mapping

| Gemini CLI Event | epic-harness Subcommand | Purpose |
|-----------------|------------------------|---------|
| BeforeAgent | `resume` | Restore session, load memory + evolved skills |
| AfterAgent | `reflect` | Analyze session, evolve skills, save metrics |
| AfterModel | `observe` (async) | Record multi-dimensional tool scores |
| BeforeModel | `guard` | Scan prompt for dangerous shell patterns |

> **Note**: Gemini CLI has no PreToolUse/PostToolUse equivalent. Guard runs at the BeforeModel
> level as a best-effort scan of prompt context for dangerous bash patterns.
