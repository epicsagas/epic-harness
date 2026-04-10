---
description: "Install guide for epic-harness Google Antigravity integration"
---

# epic-harness — Google Antigravity Integration

Connect epic-harness quality automation to Google Antigravity.

## Prerequisites

Install the epic-harness binary:

```bash
# macOS via Homebrew
brew install epic-harness

# Or via Cargo (Rust)
cargo install epic-harness
```

Verify the install:

```bash
epic-harness --version
```

## Install Steps

### 1. Copy AGENTS.md

Antigravity automatically loads `AGENTS.md` from the project root.

```bash
# Copy into your project root
cp integrations/antigravity/AGENTS.md /your-project/AGENTS.md
```

Or append the content to an existing AGENTS.md in your project.

### 2. Copy skills to .agents/skills/

```bash
mkdir -p /your-project/.agents/skills/
cp integrations/antigravity/skills/*.md /your-project/.agents/skills/
```

Skills are available to agents and auto-triggered based on context.

### 3. Copy workflows to .agents/workflows/

```bash
mkdir -p /your-project/.agents/workflows/
cp integrations/antigravity/workflows/*.md /your-project/.agents/workflows/
```

Workflows become slash commands in Antigravity: `/spec`, `/go`, `/check`, `/ship`, `/evolve`, `/team`.

### 4. Copy agents to .agents/agents/

```bash
mkdir -p /your-project/.agents/agents/
cp integrations/antigravity/agents/*.md /your-project/.agents/agents/
```

Agent personas are available for use in the Manager view.

### 5. Initialize epic-harness for the project

```bash
cd /your-project
epic-harness resume  # creates .harness/ directory and initializes
```

## Usage

**Session start** — run in terminal:
```bash
epic-harness resume
```

**Session end** — run in terminal (or use `/evolve`):
```bash
epic-harness reflect
```

**Parallel agents** — use Antigravity's Manager view to launch builder/reviewer/auditor agents in parallel during `/go` and `/check` workflows.

## Ring 0 Limitation

> **Note**: Ring 0 (auto hooks) is not available in Antigravity.
>
> Antigravity has no hooks system — there are no PreToolUse, PostToolUse, or SessionEnd events.
> The epic-harness binary (`resume`/`reflect`) must be called explicitly via terminal or workflow commands.
>
> Compensating behaviors are embedded in AGENTS.md rules and workflow steps:
> - Session startup: run `epic-harness resume` manually or via a session-start workflow
> - Session end: run `epic-harness reflect` manually or via `/evolve`
> - Guard rules: enforced through AGENTS.md forbidden commands section

## Directory Structure After Install

```
your-project/
├── AGENTS.md                    # Loaded automatically by Antigravity
├── .agents/
│   ├── skills/
│   │   ├── tdd.md
│   │   ├── secure.md
│   │   ├── verify.md
│   │   ├── simplify.md
│   │   ├── perf.md
│   │   └── document.md
│   ├── workflows/
│   │   ├── spec.md
│   │   ├── go.md
│   │   ├── check.md
│   │   ├── ship.md
│   │   ├── evolve.md
│   │   └── team.md
│   └── agents/
│       ├── builder.md
│       ├── reviewer.md
│       ├── auditor.md
│       └── planner.md
└── .harness/                    # Created by epic-harness resume
    ├── memory/
    ├── obs/
    ├── evolved/
    └── metrics.json
```
