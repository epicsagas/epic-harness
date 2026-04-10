# epic harness

**6개의 명령어. 자동 트리거 스킬. 자기 진화형.**

<p align="center">
<a href="../../README.md">English</a> | <a href="../ja/README.md">日本語</a> | <a href="../ko/README.md">한국어</a> | <a href="../de/README.md">Deutsch</a> | <a href="../fr/README.md">Français</a> | <a href="../zh-CN/README.md">简体中文</a> | <a href="../zh-TW/README.md">繁體中文</a> | <a href="../pt-BR/README.md">Português</a> | <a href="../es/README.md">Español</a> | <a href="../hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache_2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/Version-0.1.0-brightgreen.svg" alt="Version">
  <img src="https://img.shields.io/badge/Claude_Code-Plugin-purple.svg" alt="Claude Code Plugin">
  <img src="https://img.shields.io/badge/Architecture-4_Ring-orange.svg" alt="4-Ring Architecture">
  <img src="https://img.shields.io/badge/Mode-Self_Evolving-green.svg" alt="Self Evolving">
  <a href="https://buymeacoffee.com/epicsaga"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

**30개 이상의 명령어를 6개로 대체**하고, 현재 작업 맥락에 따라 **스킬을 자동으로 트리거**하며, 실패 패턴으로부터 **새로운 스킬을 스스로 진화**시키는 Claude Code 플러그인입니다. 외울 것은 적게, 키 입력당 지능은 더 높게.

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness 기능" width="100%" />
</p>

## 아키텍처: 4-Ring 모델

```
Ring 0 — 오토파일럿 (훅, 투명하게 동작)
  세션 복원, 자동 포맷, 가드레일, 관측 로깅

Ring 1 — 6개 명령어 (직접 호출)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — 자동 스킬 (컨텍스트 기반 트리거)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — 진화 (자기 개선)
  도구 사용 관측 → 실패 분석 → 스킬 자동 생성 → 게이트 → 리로드
```

## 설치

```bash
# Claude Code 플러그인 CLI
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# 또는 수동 설치
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rust 바이너리 (선택 사항, 훅 약 4배 빠름)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# crates.io에서 설치
cargo install epic-harness
# 또는 cargo-binstall 사용 (사전 빌드, 더 빠름)
cargo binstall epic-harness

# 소스에서 빌드
cargo install --path .
```

바이너리가 감지되면 훅에서 자동으로 사용합니다. 없으면 Node.js로 폴백합니다.

### 다른 도구에 설치

먼저 Rust 바이너리를 설치하고(필수), 이후 사용하는 도구에 인테그레이션을 설치합니다:

```bash
# 인테그레이션 설치 (글로벌, 기본값)
epic-harness install codex        # Codex CLI  → ~/.codex/
epic-harness install gemini       # Gemini CLI → ~/.gemini/
epic-harness install cursor       # Cursor     → ~/.cursor/
epic-harness install antigravity  # Antigravity → ~/.agents/ + AGENTS.md

# 프로젝트 로컬 설치
epic-harness install cursor --local

# 변경 없이 미리보기
epic-harness install gemini --dry-run
```

## 명령어

| 명령어 | 기능 |
|---------|-------------|
| `/spec` | 무엇을 만들지 정의 — 요구사항 명확화, 스펙 작성 |
| `/go` | 빌드 실행 — 자동 계획, TDD 서브에이전트, 병렬 실행 |
| `/check` | 검증 — 병렬 코드 리뷰 + 보안 감사 + 성능 점검 |
| `/ship` | 배포 — PR, CI, 머지 |
| `/team` | 프로젝트 맞춤 에이전트 팀 설계 |
| `/evolve` | 수동 진화 트리거 / 상태 확인 / 롤백 |

## 자동 스킬 (Ring 2)

스킬은 컨텍스트에 따라 자동으로 트리거됩니다. 직접 호출할 필요가 없습니다.

| 스킬 | 트리거 조건 |
|-------|--------------|
| **tdd** | 새로운 기능 구현 시 |
| **debug** | 테스트 실패 또는 에러 발생 시 |
| **secure** | 인증/DB/API/시크릿 코드 수정 시 |
| **perf** | 루프, 쿼리, 렌더링 코드 작업 시 |
| **simplify** | 파일이 200줄 초과이거나 복잡도가 높을 때 |
| **document** | 퍼블릭 API 추가 또는 변경 시 |
| **verify** | /go 또는 /ship 완료 전 |
| **context** | 컨텍스트 윈도우 사용률 70% 초과 시 |

## 훅 (Ring 0)

투명하게 실행됩니다. 사용자 조작이 필요 없습니다. **단일 Rust 바이너리** (`epic-harness`)의 서브커맨드로 구현되며, 바이너리가 없으면 Node.js로 폴백합니다.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| 훅 | 시점 | 동작 |
|------|------|------|
| **resume** | 세션 시작 | 컨텍스트 복원, 메모리 로드, 스택 감지 |
| **guard** | Bash 실행 전 | force-push-to-main, rm -rf /, DROP prod 차단 |
| **polish** | Edit 후 | 자동 포맷 (Biome/Prettier/ruff/gofmt) + 타입체크 |
| **observe** | 모든 도구 사용 시 | `.harness/obs/`에 로깅 |
| **snapshot** | compact 전 | `.harness/sessions/`에 상태 저장 |
| **reflect** | 세션 종료 | 실패 분석, 진화 스킬 시드, 게이트 |

## 평가 시스템 (Ring 3 핵심)

A-Evolve의 벤치마크 패턴을 Claude Code 훅 시스템에 통합합니다.

### 다차원 스코어링

모든 도구 호출은 3개 축으로 평가됩니다. 가중치는 `src/ts/common.ts` (또는 `src/hooks/common.rs`)의 `SCORE_WEIGHTS`로 설정 가능합니다:

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (기본값: 0.5)                          (기본값: 0.3)                             (기본값: 0.2)
```

| 차원 | 측정 대상 | 도구별 기준 |
|-----------|-----------------|-------------------|
| `tool_success` | 성공 여부 (0/1) | 9가지 실패 분류 |
| `output_quality` | 출력 품질 신호 (0.0-1.0) | Bash: 경고, 빈 출력. Edit: 재편집 감지 |
| `execution_cost` | 효율성 지표 (0.0-1.0) | 출력 크기, 무출력 성공 명령어 화이트리스트 |

### 실패 분류 (9가지 카테고리)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### 패턴 감지 (4가지 유형)

모든 임계값은 `src/ts/common.ts` (또는 `src/hooks/common.rs`)에서 설정 가능합니다:

| 패턴 | 감지 대상 | 상수 | 기본값 |
|---------|---------|----------|---------|
| `repeated_same_error` | 동일 에러 N회 이상 연속 발생 | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edit 성공 → 빌드/테스트 실패 | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | 동일 파일에서 N회 이상 작업 정체 | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | 동일 파일에서 Edit↔Error 반복 | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### 스킬 시드 임계값

| 트리거 | 상수 | 기본값 |
|---------|----------|---------|
| 약한 도구 (낮은 성공률) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| 약한 파일 유형 | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| 고빈도 에러 | `HIGH_FREQ_ERROR_MIN` | 5 |

### 정체 게이팅

- `STAGNATION_LIMIT` (기본값: 3) 세션 동안 개선 없음 → 진화 스킬을 최적 체크포인트로 자동 롤백
- `IMPROVEMENT_THRESHOLD` (기본값: 5%)
- 추세 추적: 선형 회귀를 통한 `improving` / `stable` / `declining` 판정
- 충돌 시 정적 스킬이 진화 스킬보다 항상 우선

### 진화 흐름

```
관측 (PostToolUse — 3축 스코어링)
    ↓ .harness/obs/session_{id}.jsonl
분석 (SessionEnd)
    ↓ SessionAnalysis: 도구별, 확장자별, 점수 분포
    ↓ 패턴: repeated_same_error, fix_then_break, long_debug_loop, thrashing
시드 (4가지 경로: 패턴 / 약한 도구 / 약한 파일 유형 / 고빈도 에러)
    ↓ .harness/evolved/{skill}/SKILL.md
게이트 (포맷 검사, 중복 제거, 10개 상한, 정체 검사)
    ↓ .harness/evolved_backup/ (최적 체크포인트)
리로드 (다음 세션 — resume.ts가 메트릭 보고 + 진화 스킬 로드)
```

```bash
/evolve              # 지금 진화 실행
/evolve status       # 대시보드: 점수, 추세, 패턴, 스킬
/evolve history      # 장기 분석: 전체 이력, 스킬 효과, 디스패치 통계
/evolve cross-project # 크로스 프로젝트 패턴 분석
/evolve rollback     # 이전 최적 상태로 복원
/evolve reset        # 모든 진화 데이터 초기화
```

## 콜드 스타트 프리셋

유용한 진화 스킬을 얻기 위해 5세션을 기다릴 필요가 없습니다. 첫 세션에서 epic harness가 스택을 감지하고 프리셋 스킬을 자동으로 적용합니다:

| 스택 | 프리셋 스킬 |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

프리셋은 보조 역할이며, 데이터가 축적되면 실제 진화 스킬로 대체됩니다.

## 동시 세션 안전성

각 세션은 자체 관측 파일(`session_{date}_{pid}_{random}.jsonl`)에 기록합니다. 같은 프로젝트에서 여러 Claude Code 세션을 동시에 사용해도 데이터가 손상되지 않습니다. reflect 훅이 당일 모든 세션 파일을 병합하여 분석합니다.

## 커스텀 가드 규칙

`.harness/guard-rules.yaml`을 통해 프로젝트별 안전 규칙을 추가할 수 있습니다:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

규칙은 내장 가드(force-push-to-main, rm -rf /, DROP prod)와 병합됩니다.

## 크로스 프로젝트 학습

프로젝트 간 실패 패턴 공유를 옵트인할 수 있습니다:

```bash
touch .harness/.cross-project-enabled  # 옵트인
```

활성화 시:
- 세션 종료 시 익명화된 패턴을 `~/.harness-global/patterns.jsonl`로 내보냄
- 세션 시작 시 다른 프로젝트의 취약 영역에 대한 힌트 표시
- `/evolve cross-project`로 종합 패턴 확인 가능

## 스킬 효과 추적

모든 진화 스킬은 A/B 기여도 점수로 추적됩니다:

```
/evolve history → Skill Effectiveness 섹션

| Skill              | Sessions | Score With | Score Without | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

양수 delta = 스킬이 효과적. 음수 delta = `/evolve rollback`으로 제거 검토.

## Polish → Observe 피드백

polish 훅(자동 포맷 + 타입체크)의 결과가 관측 파이프라인으로 피드백됩니다:

- 포맷 실패 → `lint_fail`로 기록
- TypeScript 에러 → `build_fail`로 기록
- 성공 → 전체 점수와 함께 기록

이를 통해 "편집 → 타입 에러 → 편집 → 타입 에러" 같은 쓰래싱 패턴이 수동 명령이 아닌 polish 훅에서 발생하더라도 감지됩니다.

## 프로젝트 데이터 (`.harness/`)

epic harness는 프로젝트에 `.harness/` 디렉토리를 생성합니다:

```
.harness/
├── memory/           # 프로젝트 패턴 및 규칙 (영구 보존)
├── sessions/         # 세션 스냅샷 (resume용)
├── obs/              # 도구 사용 관측 로그 (JSONL, 세션별)
├── evolved/          # 자동 진화 스킬
├── evolved_backup/   # 최적 체크포인트 (정체 시 롤백용)
├── dispatch/         # 스킬 디스패치 로그 (JSONL)
├── team/             # /team이 생성한 에이전트 및 스킬
├── evolution.jsonl   # 전체 진화 이력
├── metrics.json      # 집계 통계 + 스킬 기여도
└── guard-rules.yaml  # 커스텀 가드 규칙 (선택 사항)
```

`.harness/`를 `.gitignore`에 추가하거나 커밋하거나 — 선택은 자유입니다.

## 개발

### Rust (기본 — 약 4배 빠름)

```bash
cargo install --path .          # 빌드 + ~/.cargo/bin/에 설치
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # 플러그인 바이너리 업데이트
```

### Node.js (폴백)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### 훅 디스패치 방식

`hooks.json`의 각 훅은 세 곳에서 Rust 바이너리를 찾은 후 Node.js로 폴백합니다:

```
1. 플러그인 로컬: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (cargo install 경유)
3. 폴백:         node hooks/scripts/<hook>.js
```

### 테스트

```bash
cargo test       # 98개 Rust 단위 테스트
npm test         # Node.js 단위 + e2e 테스트
```

## 감사의 말

epic harness는 다음 프로젝트들의 아이디어에 영감을 받아 제작되었습니다:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — 자동화된 진화 및 벤치마크 패턴
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code 에이전트 스킬 시스템
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — 종합 Claude Code 패턴
- [gstack](https://github.com/garrytan/gstack) — 플러그인 아키텍처 레퍼런스
- [harness](https://github.com/revfactory/harness) — 훅 및 하네스 인프라 패턴
- [serena](https://github.com/oraios/serena) — 자율 에이전트 설계
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — 멀티 명령어 프레임워크 아키텍처
- [superpowers](https://github.com/obra/superpowers) — Claude Code 확장 패턴

## 라이선스

[Apache 2.0](LICENSE)
