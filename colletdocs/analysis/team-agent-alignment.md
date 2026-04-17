# Team 기능 ↔ Agent 지원 정합성 분석

> 분석 일시: 2026-04-17
> 분석 대상: `src/hooks/team/`, `src/hooks/install.rs`, `agents/`, `integrations/`, `references/`

---

## 1. 아키텍처 개요

이 프로젝트는 **두 개의 독립적인 에이전트 시스템**을 가지고 있다:

| 시스템 | 위치 | 역할 | 에이전트 |
|--------|------|------|----------|
| **Canonical Agents** (install.rs) | `agents/*.md` → `CANONICAL_AGENTS` | 빌트인 품질 에이전트 (builder, reviewer, auditor, planner) | 4개 고정 |
| **Team Agents** (team/) | `~/.harness/orgs/{org}/teams/{team}/agents/` | 사용자 정의 팀 에이전트 (ops, scribe, explorer 등) | 사용자 생성 |

```
install.rs (install 흐름):
  CANONICAL_AGENTS ──transform_agent(tool)──→ agents/builder.md, agents/reviewer.md ...
  대상: ~/.codex/agents/, ~/.cursor/agents/, ~/.gemini/agents/ 등

team/cli.rs (team sync 흐름):
  Team Store ──inject_team_context()──→ .claude/agents/{team}/
  대상: 오직 .claude/agents/ (Claude Code만)
```

---

## 🔴 정합성 이슈 (Consistency Issues)

### I-1. 🚨 Team Sync가 Claude Code에만 동작함 (치명적)

**문제**: `sync_to_dest()`는 **하드코딩**으로 `.claude/agents/` 경로만 생성한다.

```rust
// cli.rs L175-201
let dest = if global {
    home_dir().join(".claude").join("agents")  // ← 하드코딩
    ...
} else {
    cwd.join(".claude").join("agents").join(team)  // ← 하드코딩
};
```

반면 `install.rs`의 canonical agents는 7개 툴 모두에 동기화된다:
- ✅ codex → `~/.codex/agents/`
- ✅ gemini → `~/.gemini/agents/`
- ✅ cursor → `~/.cursor/agents/`
- ✅ opencode → `~/.config/opencode/agents/`
- ✅ claude → `.claude/agents/` (local only, canonical은 transform 없이)
- ❌ cline → 에이전트 없음 (정당한 이유: Cline은 에이전트 개념 없음)
- ❌ aider → 에이전트 없음 (정당한 이유: Aider는 에이전트 개념 없음)

**영향**: 사용자가 `epic team sync core`를 실행하면, 팀 에이전트는 Claude Code에서만 사용 가능하며, Codex/Gemini/Cursor/OpenCode에서는 **팀 에이전트가 보이지 않음**. 반대로, canonical 에이전트(builder 등)는 모든 툴에 동기화되지만 팀 에이전트(ops, scribe 등)는 아님.

### I-2. 🚨 에이전트 변환(transform_agent)이 Team Agents에 적용되지 않음

**문제**: `install.rs`의 `transform_agent()`는 CANONICAL_AGENTS(builder/reviewer/auditor/planner)에만 호출된다. Team agents(ops/scribe/explorer 및 사용자 정의)는 `inject_team_context()`만 거치고, **툴별 변환이 전혀 없다**.

| 변환 | Canonical Agents | Team Agents |
|------|-----------------|-------------|
| codex addendum (Sub-agent 호출법) | ✅ 적용 | ❌ 미적용 |
| gemini tools 리매핑 (`[Read, Edit]` → `[read_file, replace]`) | ✅ 적용 | ❌ 미적용 |
| cursor `model: inherit` 추가 | ✅ 적용 | ❌ 미적용 |
| opencode YAML tools 포맷 변환 | ✅ 적용 | ❌ 미적용 |
| Team Context 삽입 (org/team/mission) | ❌ 미적용 | ✅ 적용 |

**영향**: 사용자가 팀 에이전트의 tools 필드에 `[Read, Edit, Write, Bash, Grep, Glob]`을 쓰면 → Gemini에서는 인식 불가, OpenCode에서는 파싱 에러, Cursor에서는 model 누락.

### I-3. ⚠️ Agent 스키마 불일치

**Canonical Agent 프론트매터** (agents/*.md):
```yaml
---
name: builder
description: "..."
tools: [Read, Edit, Write, Bash, Grep, Glob]
---
```
- 필드: `name`, `description`, `tools` (3개)
- `model` 없음 (기본값 사용)

**Team Agent 프론트매터** (store.rs `build_agent_file()`):
```yaml
---
name: ops
description: ...
tools: [Read, Grep, Glob, Bash, Write, Edit]
model: sonnet
skills: [verify, secure]
---
```
- 필드: `name`, `description`, `tools`, `model`, `skills` (5개)

**불일치 항목**:
1. Canonical은 `model` 없음 → Team은 `model: sonnet` 명시
2. Canonical은 `skills` 없음 → Team의 기본 에이전트에는 `skills` 있음
3. Canonical의 builder는 `tools: [Read, Edit, Write, Bash, Grep, Glob]` → Team의 ops는 `tools: [Read, Grep, Glob, Bash, Write, Edit]` (**순서 다름**)

순서 차이는 기능적 영향은 없지만, `transform_agent()` 내 `replace()`가 정확한 문자열 매칭에 의존하므로, 사용자가 임의 순서로 tools를 작성하면 **변환이 실패**할 수 있다.

### I-4. ⚠️ Canonical Agent에 model 필드 누락

Canonical agents(builder, reviewer, auditor, planner)는 프론트매터에 `model` 필드가 없다. 하지만:
- Cursor 변환은 `model: inherit`을 **삽입**함 (L261)
- Codex/Gemini/OpenCode 변환은 `model`을 건드리지 않음
- Team agents는 `model: sonnet`을 명시함

이는 의도적일 수 있으나, 일관성을 위해 canonical에도 명시하는 것이 좋다.

### I-5. ⚠️ Gemini tools 리매핑이 문자열 매칭에 의존

```rust
// install.rs L189-201
.replace("tools: [Read, Edit, Write, Bash, Grep, Glob]",
         "tools: [read_file, replace, write_file, run_shell_command, grep_search, glob]")
.replace("tools: [Read, Grep, Glob, Bash]", ...)
.replace("tools: [Read, Grep, Glob]", ...)
```

이 방식은:
1. **정확한 문자열 매칭 필요** — 공백, 줄바꿈, 순서가 다르면 매칭 실패
2. **3개 패턴만 처리** — Canonical의 4개 에이전트 외에 새 tools 조합이 나오면 대응 불가
3. Team agents에서는 **아예 호출되지 않음** (I-2와 연계)

### I-6. ⚠️ 기본 에이전트(Team)와 Canonical 에이전트의 명명 충돌 가능성

Canonical agents: `builder`, `reviewer`, `auditor`, `planner`
Team 기본 에이전트(stream 타입): `domain-expert`, `reviewer`, `tester`

**`reviewer`가 양쪽에 모두 존재**:
- Canonical: `agents/reviewer.md` (코드 리뷰 전문)
- Team(stream): `default_agents_for_type("stream")` → `("reviewer", "Code review, quality assurance...")`

동일한 이름이지만 완전히 다른 내용. 팀 sync가 Claude Code만 지원하므로 현재는 충돌이 발생하지 않지만, 향후 다른 툴로 확장하면 파일 덮어쓰기 위험.

### I-7. ⚠️ Team Store와 Install 대상 경로가 분리됨

```
Team Store:       ~/.harness/orgs/{org}/teams/{team}/agents/     (source of truth)
Install Target:   각 툴별 디렉토리                               (canonical agents only)
Team Sync Target: .claude/agents/{team}/                         (team agents, Claude only)
```

세 경로가 모두 다르며, `epic install`이 실행될 때 Team Store는 무시되고, `epic team sync`가 실행될 때 Canonical 에이전트는 무시된다. 사용자 입장에서 "에이전트를 설치했는데 팀 에이전트는 어디 있나요?" 혼란 가능.

### I-8. ℹ️ Planner의 Gemini 변환이 문자열 치환에 과도하게 의존

```rust
// install.rs L219-249 — 8개의 개별 replace() 호출
result.replace("description: \"Breaks down a goal into ordered, parallelizable tasks...\"",
               "description: \"Breaks down a goal into ordered, sequential tasks...\"");
result.replace("5. **Parallelize**: Mark independent tasks...", ...);
result.replace("   - Parallel: yes\n", ...);
result.replace("   - Parallel: no\n", ...);
// ... 4 more replacements
```

이는 canonical 에이전트의 **본문 텍스트에 강하게 결합**되어 있어, canonical 내용이 조금만 바뀌어도 변환이 조용히 실패함 (에러 없이 원본 텍스트가 그대로 노출).

---

## 🟢 정상 작동 영역 (Areas Working Correctly)

### W-1. Canonical Agent 변환 — 4개 툴(Codex/Gemini/Cursor/OpenCode)에 대해 정상

`transform_agent()`의 핵심 변환 로직은 각 툴에 맞게 정확하게 동작:
- **Codex**: addendum 추가 ✅
- **Gemini**: tools 리매핑 + sequential 노트 + planner 전용 변환 ✅
- **Cursor**: `model: inherit` 삽입 ✅
- **OpenCode**: YAML tools 포맷 변환 + Codex addendum 재사용 ✅

테스트 커버리지도 충분:
```
test_transform_agent_codex_adds_addendum         ✅
test_transform_agent_gemini_remaps_tools         ✅
test_transform_agent_gemini_adds_note            ✅
test_transform_agent_cursor_adds_model_inherit   ✅
test_transform_agent_opencode_yaml_tools         ✅
test_transform_agent_opencode_readonly_tools     ✅
test_transform_agent_gemini_planner_sequential   ✅
```

### W-2. Team Store CRUD — 완전하고 견고함

- `list_teams`, `load_team_config`, `save_team_config` 등 CRUD 함수 일관됨
- `save_agent`의 백업 메커니즘 (`.history/` 디렉토리) 잘 설계됨
- `inject_team_context()`의 프론트매터 조작 (기존 org/team 제거 후 재삽입) 정확함
- `read_org_from_agent_file()`를 활용한 delete 시 자동 org 감지 (L726-741) 우수함

### W-3. Team CLI 인터랙티브 플로우 — 완성도 높음

- 프로젝트 스캔 → 팀 타입 추천 → 에이전트 제안 → 확인 → 저장 → sync의 전체 흐름이 갖춰짐
- 기존 팀 업데이트 시 diff 표시 및 confirm 절차 적절
- TOCTOU 방어를 위한 canonicalize 체크 (L180-191) 보안상 우수

### W-4. 기본 팀 자동 생성 — idempotent하게 동작

`install_default_team_if_needed()`는 첫 `epic install` 시 자동으로 `epic` org에 `core` 팀을 생성. 테스트도 있음:
```
test_install_default_team_idempotent  ✅
test_default_team_agents_seeded       ✅
```

### W-5. 각 툴별 Integration 파일 — team 명령어 참조 일관됨

모든 툴의 team 명령어 파일(codex/prompts/team.md, cursor/commands/team.md, opencode/commands/team.md, gemini/commands/team.toml)이 동일하게 `epic team` CLI를 래핑하고 있어 일관성 유지됨.

### W-6. 안전한 파일 쓰기 — write_if_missing / write_or_sync

- Root files (GEMINI.md 등)는 절대 덮어쓰지 않음
- settings.json은 머지 방식으로 기존 설정 보존
- 보존 파일(config.toml, .aider.conf.yml)은 사용자 커스터마이징 보호

### W-7. MCP 인젝션 — Claude/Gemini/Cursor/OpenCode 모두 지원

- Claude Code → `~/.claude.json`
- Gemini → `settings.json`
- Cursor → `mcp.json`
- OpenCode → `opencode.json` (다른 스키마: `mcp` 키 사용)

각 툴의 설정 파일 형식 차이를 정확히 반영하고 있음.

---

## 📋 개선 권장사항 (Recommended Improvements)

### P-1 (높음): Team Sync를 모든 툴에 확장

**현재**: Team sync는 `.claude/agents/`만 지원
**권장**: `ToolConfig`를 재사용하여, team sync가 `epic team sync core --tool codex` 또는 `epic team sync core --all-tools`로 모든 툴에 동기화되도록 변경

```rust
// 제안 구조
fn sync_to_tool(org: &str, team: &str, tool: &str, global: bool) -> io::Result<u32> {
    let cfg = tool_config(tool).ok_or(...)?;
    let dest = if global { &cfg.global_dir } else { &cfg.local_dir };
    let agents = list_agents(org, team);
    for agent_name in &agents {
        let content = load_agent(org, team, agent_name);
        let transformed = transform_agent(tool, agent_name, &content); // ← 핵심
        let injected = inject_team_context(&transformed, org, team, ...);
        write_to(dest.join(format!("agents/{}.md", agent_name)), injected);
    }
}
```

### P-2 (높음): Team Agents에도 transform_agent() 적용

**현재**: `sync_to_dest()`는 `inject_team_context()`만 호출
**권장**: `inject_team_context()` 전에 `transform_agent(tool, name, content)`를 먼저 호출

이렇게 하면 Team 에이전트의 tools 필드도 자동으로 각 툴에 맞게 변환됨.

### P-3 (높음): tools 변환을 구조적 파싱으로 변경

**현재**: `replace("tools: [Read, Edit, ...]", ...)` — 문자열 매칭
**권장**: YAML 프론트매터를 파싱하여 tools 배열을 구조적으로 변환

```rust
fn remap_tools_for_gemini(tools: &[&str]) -> String {
    let mapped: Vec<&str> = tools.iter().map(|t| match t {
        "Read" => "read_file", "Edit" => "replace", "Write" => "write_file",
        "Bash" => "run_shell_command", "Grep" => "grep_search", "Glob" => "glob",
        other => other,
    }).collect();
    format!("tools: [{}]", mapped.join(", "))
}
```

이렇게 하면 순서에 무관하게 동작하며, 새 tools 조합에도 자동 대응 가능.

### P-4 (중간): reviewer 이름 충돌 해결

**권장**: Canonical 에이전트의 reviewer를 `code-reviewer`로, Team의 stream 타입 기본 에이전트 reviewer를 `quality-reviewer`로 분리하거나, team 에이전트에 namespace 접두사 추가 검토.

### P-5 (중간): Canonical 에이전트에 model 필드 추가

**권장**: `agents/*.md`에 `model: sonnet`을 명시하여, Team 에이전트와 스키마 일치시킴. 현재 Cursor만 `model: inherit`을 삽입하므로, 명시적 기본값이 더 명확함.

### P-6 (중간): Planner Gemini 변환을 구조화

**권장**: 문자열 replace 8개 대신, canonical planner 자체에 tool-specific section을 마킹(`<!-- GEMINI-SEQUENTIAL -->` 등)하여, 변환 시 마킹된 섹션만 치환. 이렇게 하면 canonical 본문 수정이 변환 로직에 영향을 주지 않음.

### P-7 (낮음): Cline/Aider에 대한 에이전트 지원 검토

Cline과 Aider는 현재 에이전트 개념이 없어 canonical/team 에이전트 모두 동기화되지 않음. 이는 타당한 결정이지만, 문서에 명시하면 좋음:
- "Team agents are currently only available for Claude Code, Codex, Gemini CLI, Cursor, and OpenCode"

### P-8 (낮음): Team CLI에 --tool 플래그 추가

현재 sync 명령어:
```
epic team sync <team> [--org <name>] [--global]
```

권장:
```
epic team sync <team> [--org <name>] [--global] [--tool <tool>] [--all-tools]
```

### P-9 (낮음): Team Agent에 대한 프론트매터 스키마 검증

`build_agent_file()`에서 생성하는 에이전트는 항상 올바른 형식이지만, 사용자가 수동으로 편집할 경우 잘못된 프론트매터가 들어갈 수 있음. `load_agent()` 후 파싱 검증 로직 추가 권장.

---

## 요약 매트릭스

| 항목 | Canonical (install.rs) | Team (team/) | 정합성 |
|------|----------------------|-------------|--------|
| **에이전트 소스** | agents/*.md (빌트인) | ~/.harness/orgs/ (사용자 정의) | 분리됨 |
| **지원 툴** | 7개 (claude/codex/gemini/cursor/opencode/cline/aider) | 1개 (claude만) | 🔴 불일치 |
| **툴별 변환** | transform_agent() 4개 툴에 적용 | 없음 | 🔴 누락 |
| **Team Context 삽입** | 없음 | inject_team_context() | 🟡 단방향 |
| **프론트매터 스키마** | name/description/tools (3필드) | name/description/tools/model/skills (5필드) | 🟡 불일치 |
| **명명 충돌** | reviewer 있음 | stream 타입에 reviewer 있음 | 🟡 위험 |
| **CRUD/백업** | 없음 (재설치만) | save/load/list/history 완비 | 🟢 Team이 우수 |
| **MCP 인젝션** | 툴별 차이 정확 반영 | 해당 없음 | 🟢 정상 |
| **테스트** | 변환/설치 20개 테스트 | idempotent/seeded 2개 테스트 | 🟡 Canonical이 우수 |
