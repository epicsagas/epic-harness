# PR #3 Configuration & Documentation — Deployment Readiness Analysis

> **Date**: 2026-04-18
> **Scope**: Team feature (`/team` / `epic team`) — 13 file categories analyzed

---

## Executive Summary

**⚠️ NOT READY FOR RELEASE** — 2 blockers, 3 high-severity issues, several minor inconsistencies.

The team feature is well-documented across integrations and i18n locales, but version inconsistencies, a missing spec file, and an empty CHANGELOG entry block a clean release.

---

## Per-File Analysis

### 1. `Cargo.toml` — ⚠️ Version Issue

| Check | Status |
|-------|--------|
| Binary targets | ✅ Two binaries (`epic-harness`, `epic`) + lib (`epic_harness`) |
| Dependencies | ✅ All reasonable (serde, regex, crossterm, rusqlite, tiny_http, etc.) |
| Release profile | ✅ LTO + strip + opt-level="s" |
| binstall metadata | ✅ Proper tar.gz/zip URLs |
| **Version** | ⚠️ **`0.1.10-dev`** — pre-release suffix, not a release version |
| Edition | ✅ `2024` (valid since Rust 1.85+) |

**Issue**: The `-dev` suffix indicates this is a development build. For release, should be `0.1.10` or similar.

---

### 2. `CHANGELOG.md` — 🔴 BLOCKER

| Check | Status |
|-------|--------|
| Has `[Unreleased]` section | ✅ Yes |
| **Team feature mentioned** | 🔴 **NO — zero mentions of "team"** |
| Previous versions | ✅ 0.1.1 through 0.1.3 documented |

The entire `[Unreleased]` section covers the Unified Memory system, opencode/cline/aider integrations, and install improvements. **The `/team` command addition is completely absent** from the changelog. This is a release blocker.

**Missing entries**:
- `/team` command (new)
- Team storage model (`~/.harness/orgs/`)
- Team subcommands (list, show, sync, link, unlink, delete, history)
- Team types (stream, platform, enabling, subsystem)
- Merge strategy
- Multi-org support

---

### 3. `README.md` — ✅ Well Updated

| Section | Status |
|---------|--------|
| Commands table includes `/team` | ✅ Line 239 |
| Ring 1 mentions `/team` | ✅ Line 31 |
| `## Team (epic team)` section | ✅ Lines 242–345 |
| How it works | ✅ Flow diagram present |
| CLI reference | ✅ All subcommands listed |
| Team types table | ✅ 4 types with keywords |
| Merge strategy | ✅ "no silent overwrites" table |
| Multi-org | ✅ Examples with `--org` |
| Team Context injection | ✅ Markdown example |
| **Version badge** | ⚠️ Shows `0.1.0` (stale) |

**Minor**: Version badge `0.1.0` is outdated. Should match the release version.

---

### 4. `package.json` — ⚠️ Version Drift

| Field | Value |
|-------|-------|
| version | **`0.1.4`** |
| files array | ✅ Includes commands/, skills/, agents/, etc. |
| No team-specific entry needed | ✅ files array covers `commands/` which includes team.md |

**Issue**: Version `0.1.4` doesn't match Cargo.toml `0.1.10-dev` or plugin.json `0.1.1`.

---

### 5. `.claude-plugin/plugin.json` — ⚠️ Version Drift

| Field | Value |
|-------|-------|
| version | **`0.1.1`** |
| name | `epic` |

**Issue**: Version `0.1.1` is the oldest of the three. Should be updated to match.

---

### 6. `docs/team.md` — ✅ Comprehensive

| Check | Status |
|-------|--------|
| Overview | ✅ Core model diagram |
| Storage layout | ✅ Directory tree with config.json schema |
| Usage (interactive) | ✅ 4-phase flow described |
| Subcommands | ✅ All 8 listed with flags |
| Team types | ✅ 4 types with default agents |
| Merge strategy | ✅ Per-object rules |
| Project integration | ✅ Team Context injection |
| Multi-org | ✅ Example |
| Implementation reference | ✅ src/hooks/team/ modules |
| **Spec reference** | 🔴 References `docs/research/team-spec.md` — **file does not exist** |

---

### 7. `commands/team.md` — ✅ Complete

| Check | Status |
|-------|--------|
| Frontmatter description | ✅ |
| CLI reference | ✅ Includes `epic org` commands too |
| Process (5 phases) | ✅ Browse → Hire → Design → Generate → Link |
| Team patterns | ✅ Pipeline, Fan-out, Expert Pool, etc. |
| Constraints | ✅ Max 6 agents, clear boundaries |
| Red flags | ✅ Good list |
| **Spec reference** | ⚠️ No broken ref (doesn't reference team-spec.md) |

**Note**: This is more detailed than the integration wrappers — it's the Claude Code command with the full process/decision guide. Has `epic org list/show` commands not documented elsewhere.

---

### 8. `integrations/codex/prompts/team.md` — ⚠️ Broken Reference

| Check | Status |
|-------|--------|
| Wrapper content | ✅ Correct (delegates to `epic team`) |
| Subcommands listed | ✅ 6 subcommands |
| **Spec reference** | 🔴 `docs/research/team-spec.md` — **file does not exist** |

---

### 9. `integrations/cursor/commands/team.md` — ⚠️ Broken Reference

| Check | Status |
|-------|--------|
| Wrapper content | ✅ Identical to codex (correct) |
| **Spec reference** | 🔴 `docs/research/team-spec.md` — **file does not exist** |

---

### 10. `integrations/gemini/commands/team.toml` — ⚠️ Broken Reference

| Check | Status |
|-------|--------|
| TOML format | ✅ Correct structure |
| Wrapper content | ✅ Same delegation pattern |
| **Spec reference** | 🔴 `docs/research/team-spec.md` — **file does not exist** |

---

### 11. `integrations/opencode/commands/team.md` — ⚠️ Broken Reference

| Check | Status |
|-------|--------|
| Wrapper content | ✅ Same as cursor/codex (slightly shorter intro) |
| **Spec reference** | 🔴 `docs/research/team-spec.md` — **file does not exist** |

---

### 12. i18n Files — ✅ Complete Coverage

All 9 locales include the team feature:

| Locale | `/team` in commands | Team section | Team Types table | Merge strategy | Version badge |
|--------|---------------------|-------------|-----------------|----------------|---------------|
| 🇩🇪 de | ✅ | ✅ Teams | ✅ (in Team Types section) | ✅ Merge-Strategie | ⚠️ 0.1.0 |
| 🇪🇸 es | ✅ | ✅ Equipos | ✅ Tipos de equipo | ✅ Estrategia de fusión | ⚠️ 0.1.0 |
| 🇫🇷 fr | ✅ | ✅ Équipes | ✅ Types d'équipes | ✅ Stratégie de fusion | ⚠️ 0.1.0 |
| 🇮🇳 hi | ✅ | ✅ टीम | ✅ टीम प्रकार | ✅ मर्ज रणनीति | ⚠️ 0.1.0 |
| 🇯🇵 ja | ✅ | ✅ チーム | ✅ (implied in section) | ✅ マージ戦略 | ⚠️ 0.1.0 |
| 🇰🇷 ko | ✅ | ✅ 팀 | ✅ (implied in section) | ✅ 병합 전략 | ⚠️ 0.1.0 |
| 🇧🇷 pt-BR | ✅ | ✅ Equipes | ✅ Tipos de equipe | ✅ Estratégia de mesclagem | ⚠️ 0.1.0 |
| 🇨🇳 zh-CN | ✅ | ✅ 团队 | ✅ (in section) | ✅ 合并策略 | ⚠️ 0.1.0 |
| 🇹🇼 zh-TW | ✅ | ✅ 團隊 | ✅ 團隊類型 | ✅ 合併策略 | ⚠️ 0.1.0 |

**All locales**: Version badge consistently shows `0.1.0` (all stale in sync).

---

### 13. `references/security.md` — ✅ Updated for Team

| Check | Status |
|-------|--------|
| OWASP Top 10 | ✅ Complete checklist |
| **Team-specific security** | ✅ Lines 64–110: new "LLM / Agent File Security" section |
| Unicode prompt injection | ✅ Blocked ranges documented |
| YAML frontmatter injection | ✅ Mitigations described |
| HTML comment injection | ✅ Playbook-specific mitigations |
| Path traversal via agent names | ✅ `validate_agent_name` documented |
| ANSI injection via filenames | ✅ `is_ascii_graphic()` sanitization |

This is well-done — security.md has been proactively updated with team-specific attack vectors.

---

## Cross-File Issues

### 🔴 Version Inconsistency (Blocker)

| File | Version |
|------|---------|
| Cargo.toml | `0.1.10-dev` |
| package.json | `0.1.4` |
| plugin.json | `0.1.1` |
| README badge (all 10) | `0.1.0` |

**Four different version numbers across the project.** All must converge before release.

### 🔴 Missing `docs/research/team-spec.md`

Referenced by:
- `docs/team.md` (line 4)
- `integrations/codex/prompts/team.md`
- `integrations/cursor/commands/team.md`
- `integrations/gemini/commands/team.toml`
- `integrations/opencode/commands/team.md`

The `docs/research/` directory does not exist. Either create the spec file or remove/fix the references.

### ⚠️ `commands/team.md` documents `epic org` subcommands

The Claude Code command file references `epic org list`, `epic org show`, `epic org help` — these are not documented in `docs/team.md`, the README, or any other integration file. If these are implemented, they need documentation. If not, this is misleading.

### ⚠️ CHANGELOG Missing Team Entries

Zero mentions of team in `[Unreleased]`. This is the biggest documentation gap for users upgrading.

### Minor: Cline has partial team reference

`integrations/cline/rules/epic-harness.md` mentions `/team` in a table, but cline has no command files (expected — cline doesn't support custom commands).

---

## Release Readiness Verdict

| Category | Status |
|----------|--------|
| Feature code (src/hooks/team/) | ✅ Implemented (3 files) |
| Command spec (commands/team.md) | ✅ Comprehensive |
| Integration wrappers (4 tools) | ✅ Present |
| Main README | ✅ Well-documented |
| i18n (9 locales) | ✅ All translated |
| Security (references/security.md) | ✅ Updated |
| Docs (docs/team.md) | ✅ Detailed |
| **Version consistency** | 🔴 **4 different versions** |
| **CHANGELOG** | 🔴 **No team entry** |
| **Missing spec file** | 🟡 5 broken references |
| **README version badge** | ⚠️ Stale `0.1.0` everywhere |

### Required Before Release:

1. **Add team entries to CHANGELOG.md** `[Unreleased]` section
2. **Align all versions** — pick one version (e.g. `0.2.0` for team feature) and update Cargo.toml, package.json, plugin.json, and all README badges
3. **Either create `docs/research/team-spec.md` or remove references to it** from 5 files
4. **Clarify `epic org` commands** — document or remove from commands/team.md

### Recommended But Not Blocking:

5. Update README version badges from `0.1.0` to the release version
6. Remove `-dev` suffix from Cargo.toml version before tagging
