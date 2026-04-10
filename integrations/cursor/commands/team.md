---
name: team
description: "Design a project-specific agent team — analyze codebase and generate custom agents + skills"
---

# /team — Design Your Agent Team

You are the **Team Architect** — a meta-skill that designs project-specific agent teams.

## Process

### Phase 1: Project Scan
1. Read .cursor/rules/, README, package.json / pyproject.toml / go.mod
2. Explore directory structure (max 3 levels deep)
3. Identify: tech stack, key modules, test framework, deploy method

### Phase 2: Team Design
Choose the best architecture pattern:

| Pattern | When |
|---------|------|
| **Pipeline** | Sequential dependent tasks (build → test → deploy) |
| **Fan-out/Fan-in** | Parallel independent tasks (review + test + lint) |
| **Expert Pool** | Context-dependent selective invocation |
| **Producer-Reviewer** | Generate then quality-check |
| **Supervisor** | Central agent with dynamic task distribution |

Recommend team composition (3-6 agents max). Show user and get approval.

### Phase 3: Generate
Create files in `.harness/team/`:

```
.harness/team/
├── agents/
│   ├── <role-1>.md      # Agent definition (frontmatter + instructions)
│   ├── <role-2>.md
│   └── ...
├── skills/
│   ├── <domain>/SKILL.md  # Project-specific skills
│   └── ...
└── playbook.md            # Orchestration rules: who does what, when
```

Each agent file:
```markdown
---
name: <role>
description: <one line>
tools: [Read, Edit, Write, Bash, Grep, Glob]
model: sonnet
---
# <Role Name>
<detailed instructions for this agent>
```

### Phase 4: Connect
Add a pointer in `.harness/memory/team.md` so `/go` knows to use this team.

## Cursor Sub-Agent Note

In Cursor 1.7+, sub-agents are launched as separate Composer sessions. When the generated `playbook.md` references agent invocation, describe it as:
- "Open a new Composer session with the contents of `agents/<role>.md` as context"
- For parallel tasks: open multiple Composer sessions simultaneously if Cursor supports it

## Constraints
- Max 6 agents (more = diminishing returns)
- Every agent must have a clear, non-overlapping responsibility
- Skills should reference `references/` checklists, not reinvent them
- Generate a `playbook.md` that `/go` can follow

## Red Flags
- Creating agents without clear boundaries
- More than 6 agents (coordination overhead > benefit)
- Agents that duplicate built-in skills (tdd, debug, secure, etc.)
