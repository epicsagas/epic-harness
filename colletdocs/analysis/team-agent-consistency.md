# 팀(Team) 기능과 에이전트(Agent) 지원 간 정합성 분석

**날짜**: 2026-04-17
**대상 파일**:
- `src/hooks/team/store.rs` — 팀 데이터 스토어
- `src/hooks/team/cli.rs` — 팀 CLI 명령
- `src/hooks/install.rs` — 도구별 에이전트 변환/설치
- `agents/*.md` — 정적 에이전트 (builder, auditor, planner, reviewer)
- `integrations/*/commands/team.*` — 도구별 /team 명령

---

## 1. 아키텍처 개요

```
┌──────────────────────────────────────────────────────────────┐
│                  팀 에이전트 (Team Agents)                      │
│  저장: ~/.harness/orgs/{org}/teams/{team}/agents/*.md         │
│  생성: build_agent_file() → 범용 frontmatter + 마크다운        │
│  동기화: sync_to_dest() → .claude/agents/{team}/*.md          │
│  컨텍스트 주입: inject_team_context() → org/team/mission       │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│              정적 에이전트 (Canonical Agents)                   │
│  저장: agents/{builder,auditor,planner,reviewer}.md           │
│  설치: generate_canonical_files() → transform_agent()         │
│  대상: .codex/, .gemini/, .cursor/, .opencode/                │
│  변환: 도구별 tools 리맵핑, addendum, model: inherit           │
└──────────────────────────────────────────────────────────────┘
```

두 시스템은 **완전히 별도의 경로**로 관리됨.

---

## 2. 정합성 이슈 (Consistency Issues)

### 🔴 Critical

#### C-1. 팀 에이전트 sync 경로가 Claude 전용 (`.claude/agents/`)
- `sync_to_dest()`는 항상 `.claude/agents/{team}/`에만 동기화
- Codex (`.codex/agents/`), Gemini (`.gemini/agents/`), Cursor (`.cursor/agents/`), OpenCode 등 **다른 도구로는 팀 에이전트가 전달되지 않음**
- `install.rs`의 `generate_canonical_files()`는 정적 에이전트(builder 등)만 변환/설치
- **영향**: 팀 기능을 Codex/Gemini/Cursor/OpenCode에서 사용 불가

#### C-2. 팀 에이전트에 `transform_agent()` 변환이 적용되지 않음
- 정적 에이전트는 `transform_agent()`를 통해 도구별 변환 수행:
  - **Codex**: addendum 추가
  - **Gemini**: tools 리맵핑 (`Read` → `read_file` 등), 플래너 순차 실행 수정
  - **Cursor**: `model: inherit` 삽입
  - **OpenCode**: tools 배열 → dict 포맷 변환, 읽기 전용 에이전트 처리
- 팀 에이전트는 `inject_team_context()`만 수행하고 `transform_agent()`를 거치지 않음
- **영향**: Gemini에서 팀 에이전트의 tools 필드가 인식되지 않아 기능 장애 가능

#### C-3. Gemini `/team` 명령이 구식 (Legacy) 경로 사용
- `integrations/gemini/commands/team.toml`의 프롬프트:
  ```
  Create files in `$HARNESS_DIR/team/` (agents, skills, playbook.md)
  ```
- 실제 팀 데이터는 `~/.harness/orgs/{org}/teams/{team}/`에 저장
- `$HARNESS_DIR/team/` 경로는 현재 아키텍처와 불일치
- **반면** Codex/Cursor/OpenCode의 team 명령은 `epic team` CLI를 호출하는 래퍼로 올바름
- **영향**: Gemini 사용자가 `/team` 실행 시 엉뚱한 위치에 파일 생성

### 🟡 Medium

#### M-1. 팀 에이전트 frontmatter 스키마 불일치
- **정적 에이전트** frontmatter:
  ```yaml
  name: builder
  description: "..."
  tools: [Read, Edit, Write, Bash, Grep, Glob]
  ```
- **팀 에이전트** frontmatter (`build_agent_file()`):
  ```yaml
  name: {role}
  description: {description}
  tools: [Read, Edit, Write, Bash, Grep, Glob]
  model: sonnet
  ```
- **sync 후** 팀 에이전트 frontmatter (`inject_team_context()`):
  ```yaml
  name: {role}
  description: {description}
  tools: [Read, Edit, Write, Bash, Grep, Glob]
  model: sonnet
  org: {org}
  team: {team_name}
  ```
- `model: sonnet`이 정적 에이전트에는 없고 팀 에이전트에만 있음
- `org`, `team` 필드는 sync 시 주입되지만, 원본 저장소의 에이전트 파일에는 없음
- **영향**: 일관성 없는 스키마 → 파싱/검증 로직이 한쪽만 처리

#### M-2. tools 목록 하드코딩
- `build_agent_file()`는 모든 팀 에이전트에 동일한 tools `[Read, Edit, Write, Bash, Grep, Glob]` 할당
- 정적 에이전트는 역할별로 다른 tools 사용:
  - `auditor`: `[Read, Grep, Glob, Bash]` (Write/Edit 없음)
  - `planner`: `[Read, Grep, Glob]` (Bash도 없음)
  - `builder`/`reviewer`: `[Read, Edit, Write, Bash, Grep, Glob]`
- `default_agents_for_type()`의 "enabling" 타입은 specialist 1개만 있는데도 Write/Edit 권한 부여
- **영향**: 감사/탐색 역할의 에이전트가 파일 수정 가능 → 최소 권한 원칙 위반

#### M-3. Gemini tools 리맵핑이 팀 에이전트에 미작동
- `transform_agent("gemini", ...)`는 정확한 문자열 매칭으로 tools 리맵핑:
  ```
  "tools: [Read, Edit, Write, Bash, Grep, Glob]"
  → "tools: [read_file, replace, write_file, run_shell_command, grep_search, glob]"
  ```
- 팀 에이전트에 `model: sonnet` 줄이 추가로 있어 순서가 다름 → 매칭 실패 가능
- **영향**: Gemini에서 팀 에이전트가 인식 불가

#### M-4. default 팀 에이전트 (ops/scribe/explorer)와 정적 에이전트 (builder/auditor/planner/reviewer) 역할 중복 누락
- 정적 에이전트: builder, auditor, planner, reviewer
- 기본 팀 에이전트: ops, scribe, explorer
- **겹치는 역할 없음** — auditor ↔ reviewer는 비슷하지만 별개
- `install_default_team_if_needed()`로 설치되는 core 팀은 정적 에이전트를 포함하지 않음
- **영향**: `epic install`로 설치한 정적 에이전트와 `epic team`으로 만든 팀 에이전트가 서로 다른 에이전트 세트를 제공 → 사용자 혼란

### 🟢 Low

#### L-1. `team_exists()`가 디렉토리만 확인
- `team_store_dir(org, team).is_dir()`로 존재 여부 판단
- `config.json`이 손상되거나 없어도 "존재함"으로 처리
- **영향**: 손상된 팀에 대해 오퍼레이션 시 런타임 에러

#### L-2. `read_org_from_agent_file()`이 실제로 사용되지 않음
- `store.rs`에 정의되어 있지만 CLI에서 호출하지 않음
- sync 시 이미 org를 알고 있으므로 불필요
- **영향**: 데드 코드

#### L-3. `to_title_case()`가 non-ASCII 문자를 처리하지 않음
- 한국어/일본어 팀명이나 역할명 입력 시 첫 글자 대문자 변환이 무의미
- **영향**: 비영어권 사용자 경험 저하

---

## 3. 정상 작동 영역

| 영역 | 상태 | 비고 |
|------|------|------|
| 팀 CRUD (생성/조회/수정/삭제) | ✅ | store.rs의 저장/불러오기 정상 |
| Claude Code 동기화 | ✅ | `.claude/agents/{team}/` sync 정상 작동 |
| 팀 컨텍스트 주입 | ✅ | org/team/mission/playbook 주입 로직 정상 |
| 정적 에이전트 설치 (Codex/Gemini/Cursor/OpenCode) | ✅ | `transform_agent()` 도구별 변환 정상 |
| 에이전트 히스토리 백업 | ✅ | `.history/` 디렉토리 관리 정상 |
| 팀 타입별 기본 에이전트 제안 | ✅ | stream/platform/enabling/subsystem별 차별화 |
| 팀-프로젝트 링킹 | ✅ | config.json의 projects 배열 관리 |
| CLI 입력 검증 | ✅ | `validate_team_name()` 정상 |
| TOCTOU 방어 (경로 순회) | ✅ | canonicalize + starts_with 체크 |
| 병렬 테스트 안전성 | ✅ | HOME_LOCK 뮤텍스로 직렬화 |

---

## 4. 개선 권장사항

### P0 (즉시)

1. **Gemini team.toml 업데이트**
   - `integrations/gemini/commands/team.toml`을 다른 도구와 동일하게 `epic team` CLI 래퍼로 변경
   - 구식 `$HARNESS_DIR/team/` 경로 제거

2. **`sync_to_dest()`에 `transform_agent()` 적용**
   - 팀 에이전트 sync 시 대상 도구 감지 후 변환 수행
   - 최소한 tools 리맵핑은 필수 (Gemini, OpenCode)

### P1 (단기)

3. **팀 에이전트 동기화 대상 확장**
   - `epic team sync --tool codex` 등 도구별 동기화 지원
   - 또는 `epic install` 시 팀 에이전트도 포함하는 옵션

4. **`build_agent_file()` tools 매개변수화**
   - 역할별 최소 권한 tools 세트 정의
   - auditor/explorer 타입에 Write/Edit 제외

5. **정적 에이전트와 기본 팀 에이전트 통합 고려**
   - core 팀에 builder/auditor/planner/reviewer를 포함하거나
   - 문서에서 두 시스템의 관계를 명확히 설명

### P2 (중기)

6. **통합 에이전트 스키마 정의**
   - 정적/팀 에이전트 공통 스키마 수립
   - 선택적 필드: `model`, `org`, `team`, `skills`
   - 필수 필드: `name`, `description`, `tools`

7. **`team_exists()` 검증 강화**
   - 디렉토리 + config.json 존재 확인
   - 손상된 팀 자동 감지/복구

8. **`read_org_from_agent_file()` 활용 또는 제거**
   - sync된 에이전트에서 org 역추적 기능이 필요하면 활용
   - 불필요하면 데드 코드 제거

---

## 5. 요약

| 등급 | 개수 | 주요 내용 |
|------|------|----------|
| 🔴 Critical | 3 | 팀 에이전트가 Claude 외 도구에 전달되지 않음, Gemini team 명령 구식 |
| 🟡 Medium | 4 | 스키마 불일치, tools 하드코딩, 역할 중복 누락 |
| 🟢 Low | 3 | 검증 약화, 데드 코드, non-ASCII 처리 |
| ✅ 정상 | 10+ | 팀 CRUD, Claude 동기화, 컨텍스트 주입, 보안 방어 등 |

**핵심 문제**: 팀 에이전트 시스템과 `install.rs`의 정적 에이전트 시스템이 **완전히 분리**되어 있어, 팀 기능은 사실상 Claude Code 전용으로 동작함. 다른 도구(Codex, Gemini, Cursor, OpenCode) 사용자는 `epic team`으로 만든 에이전트를 사용할 수 없음.
