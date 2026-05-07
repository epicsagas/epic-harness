---
name: reflect
description: "AI를 사고 증폭기로 사용하고 있는지 냉정하게 성찰. 세션 기록·obs·evolution·memory를 근거로 5개 차원(사고증폭/자기개선/메타인지확장/프롬프트개선/실행효율) 점수와 액션 아이템을 도출. 트리거: /reflect 또는 주기적 자기 성찰 시."
---

# Reflect — AI 사고 증폭기 사용 성찰

## Iron Law

증거 없는 점수는 없다. 모든 평가 항목은 obs 통계·evolution 패턴·memory 노드·session 요약 중 최소 하나를 직접 인용해야 한다.
긍정 편향(self-serving bias)을 차단: "잘 하고 있다"는 결론은 구체적 수치 없이 쓸 수 없다.

## Process

### Step 0 — 컨텍스트 수집

```bash
# HARNESS_DIR은 환경변수로 미리 설정되어 있어야 함
# 없으면: export HARNESS_DIR=$(epic-harness path)
SCRIPT="$HARNESS_DIR/../../../scripts/reflect-context.sh"
bash "$SCRIPT" 30 > /tmp/reflect_ctx.json
```

실패하면 직접 수집:
```bash
echo "obs_files: $(ls "$HARNESS_DIR/obs/" | wc -l)"
python3 -c "import json; m=json.load(open('$HARNESS_DIR/metrics.json')); print('total_sessions:', m.get('total_sessions',0))"
```

harness-mem에서 관련 기억 조회 (도구가 활성화된 경우):
```
mem_recall(hint="AI usage patterns decisions metacognition", limit=8)
mem_query(type="decision", limit=5)
mem_query(type="pattern", limit=5)
```

### Step 1 — 5개 차원 성찰

각 차원을 독립적으로 평가한다. 점수(1–10) + 증거 인용 + 한 줄 진단.

#### 차원 1: 사고 증폭 (Thought Amplification)
**묻는 것**: AI가 단순 실행기(코드 타이핑)인가, 진짜 사고 파트너인가?

평가 지표:
- Agent 도구 호출 비율 (`Agent / total_obs` — 높을수록 위임 사고)
- Skill 호출 빈도 (메타 레이어 활용 여부)
- council/discover/spec 실행 이력
- harness-mem decisions 노드 수

진단 기준:
| 점수 | 신호 |
|------|------|
| 8–10 | Agent 위임 비율 ≥ 5%, Skill 다양하게 사용, council 실행 기록 있음 |
| 5–7  | Bash/Read/Edit 위주, Agent 간헐적, 복잡한 결정은 혼자 내림 |
| 1–4  | Bash+Edit 90%+, AI가 코드 자동완성 수준에 머묾 |

#### 차원 2: 자기 개선 (Self-Improvement)
**묻는 것**: 실수에서 학습하고 있는가, 같은 패턴을 반복하는가?

평가 지표:
- `evolution_stats.pattern_frequency` — 같은 패턴이 반복되는가?
- `evolution_stats.stagnation_count` — 정체 세션 수
- `evolution_stats.trend_last10` — improving/stable/declining 분포
- evolved skills 누적 수 vs 기간

진단 기준:
| 점수 | 신호 |
|------|------|
| 8–10 | trend improving 비율 60%+, 동일 패턴 재발 없음, evolved skills 증가 중 |
| 5–7  | trend stable 위주, 일부 패턴 반복, evolved skills 정체 |
| 1–4  | trend declining 또는 stagnation 다수, 동일 실수 반복 |

#### 차원 3: 메타 인지 확장 (Metacognitive Expansion)
**묻는 것**: AI와의 대화를 통해 자신의 사고 방식을 인식하고 업그레이드하고 있는가?

평가 지표:
- harness-mem의 concept/pattern 노드 수 및 최신성
- alcove decisions/ADR 기록 빈도
- session snapshot에 "배운 것" 언급 여부
- `/discover` `/spec` 실행 이력 (문제 정의 연습)

진단 기준:
| 점수 | 신호 |
|------|------|
| 8–10 | decisions 노드 정기 생성, /discover /spec 활용, ADR 존재 |
| 5–7  | 간헐적 기록, 결정 근거가 코드에만 있고 명시적 메모 없음 |
| 1–4  | 메모리 노드 거의 없음, 맥락이 세션 간 단절됨 |

#### 차원 4: 프롬프트 개선 (Prompt Engineering)
**묻는 것**: 프롬프트 품질이 시간이 지남에 따라 나아지고 있는가?

평가 지표:
- `output_quality` dimension average 추세 (metrics score_history)
- `tool_success` rate 추세
- evolved skills의 프롬프트 개선 패턴
- 세션 평균 score 추이 (초기 vs 최근)

진단 기준:
| 점수 | 신호 |
|------|------|
| 8–10 | output_quality ≥ 0.80, score 추세 우상향, evolved prompts 증가 |
| 5–7  | output_quality 0.65–0.80, 개선 정체 |
| 1–4  | output_quality < 0.65, 추세 하락, 반복 수정 많음 |

#### 차원 5: 실행 효율 (Execution Efficiency)
**묻는 것**: AI를 통해 동일 목표를 더 빠르고 적은 비용으로 달성하고 있는가?

평가 지표:
- `execution_cost` dimension average (이미 1.0이면 효율 최적)
- Bash 편중도 (Bash > 50% 이면 저수준 반복 작업 과다)
- 컨텍스트 압축(compaction) 빈도 (너무 잦으면 컨텍스트 낭비)
- Agent 서브에이전트 병렬 활용 여부

진단 기준:
| 점수 | 신호 |
|------|------|
| 8–10 | execution_cost ≥ 0.90, Agent 병렬 사용, 압축 세션 < 총 세션 20% |
| 5–7  | Bash 편중 40–60%, 단일 에이전트 직렬 실행 위주 |
| 1–4  | Bash 70%+, 서브에이전트 미활용, 단순 반복 작업 AI에 위임 |

### Step 2 — 종합 점수표 출력

```
## AI 사고 증폭기 성찰 리포트
생성일: {ISO-8601}  |  분석 기간: {N}일  |  총 세션: {total_sessions}

| 차원 | 점수 | 등급 | 핵심 증거 |
|------|------|------|----------|
| 사고 증폭      | X/10 | 🔴/🟡/🟢 | Agent {N}회 ({P}%), Skill {M}회 |
| 자기 개선      | X/10 | 🔴/🟡/🟢 | trend={T}, stagnation={S}회 |
| 메타 인지 확장 | X/10 | 🔴/🟡/🟢 | decisions {D}개, 세션 메모 {M}건 |
| 프롬프트 개선  | X/10 | 🔴/🟡/🟢 | output_quality={Q}, 추세={Δ} |
| 실행 효율      | X/10 | 🔴/🟡/🟢 | execution_cost={C}, Bash비율={B}% |
| **종합**       | **X/10** | | |

등급: 🟢 8–10 (좋음)  🟡 5–7 (보통)  🔴 1–4 (개선 필요)
```

### Step 3 — 냉정한 총평

3–5문장. 다음 규칙 적용:
- 가장 낮은 점수 차원을 첫 문장으로 시작
- "잘 하고 있다"류 문장은 수치 증거 없이 사용 금지
- 현재 가장 큰 병목 1개를 명시
- 사용 방식이 "실행 자동화"인지 "사고 증폭"인지 분류

### Step 4 — 액션 아이템 (최소 3개)

각 항목: `[우선순위] 제목 — 구체적 행동 — 기대 효과`

형식:
```
### 다음 성찰 액션

1. [HIGH] {제목}
   - 행동: {구체적 단계}
   - 지표: {어떻게 측정할 것인가}
   - 기한: {세션 수 또는 날짜}

2. [MED] ...
3. [LOW] ...
```

권장 액션 풀 (차원 점수에 따라 선택):
- 사고 증폭 낮음 → council mode 주 1회 실행, /spec 작성 습관화
- 자기 개선 낮음 → `/evolve history` 주기 검토, 반복 패턴 수동 메모
- 메타 인지 낮음 → mem_add(type=decision) 매 중요 결정 직후 실행
- 프롬프트 낮음 → 저품질 세션 직후 프롬프트 리뷰 → 개선 evolved skill 시딩
- 실행 효율 낮음 → Agent 병렬 서브에이전트 패턴 도입, 반복 Bash 스크립트화

### Step 5 — 메모리 저장

성찰 결과를 harness-mem에 저장 (mem 도구가 활성화된 경우):
```
mem_add(
  type="session",
  title="AI usage reflection {date}",
  tags=["reflection", "metacognition"],
  importance=0.8,
  body="종합: {score}/10. 최저: {lowest_dim}. 주요 액션: {top_action}"
)
```

---

## Anti-Rationalization

| 핑계 | 반박 | 대신 해야 할 것 |
|------|------|----------------|
| "세션 수가 적어서 성찰이 부정확하다" | 3개 세션이라도 패턴은 드러난다. 데이터 부족은 낮은 점수의 이유가 아니다. | 있는 데이터로 성찰하고, 데이터 수집 개선을 액션 아이템으로 기록하라. |
| "Bash가 많은 건 Rust 프로젝트라서 당연하다" | 도구 사용 분포는 작업 특성만 반영하지 않는다. Agent 위임 가능 작업을 Bash로 처리하는 패턴을 점검하라. | Bash 호출 중 Agent로 위임 가능한 것을 3개 이상 찾아라. |
| "score 0.75면 충분히 좋다" | 0.75는 절대 기준이 아니다. 이전 기간 대비 추세가 더 중요하다. | score_history에서 최근 5세션 평균 vs 이전 5세션 평균을 비교하라. |
| "memory가 없어도 컨텍스트가 있으니 괜찮다" | 컨텍스트는 세션 종료 시 사라진다. 세션 간 학습 연속성이 없으면 매번 같은 곳에서 시작한다. | 중요 결정 직후 mem_add를 실행하는 습관을 지금 시작하라. |
| "이미 코드가 잘 나오고 있으니 괜찮다" | 코드 출력 품질 ≠ 사고 증폭. 좋은 코드가 나오더라도 AI가 대신 생각하고 있을 수 있다. | 마지막 5개 세션에서 내가 직접 설계한 결정이 몇 개인지 세어라. |

## Evidence Required

- [ ] `/tmp/reflect_ctx.json` 또는 직접 수집 데이터 존재
- [ ] 5개 차원 각각에 수치 증거 1개 이상 인용
- [ ] 종합 점수표 출력 완료
- [ ] 냉정한 총평에 구체적 병목 1개 명시
- [ ] 액션 아이템 최소 3개, 각각 측정 지표 포함
- [ ] 낮은 점수 차원(1–4)에 대해 Anti-Rationalization 테이블 적용

## Red Flags

- 모든 차원 7점 이상 → 긍정 편향 의심. 각 점수를 다시 증거로 검증하라.
- 액션 아이템이 "더 자주 사용하자" 수준 → 구체적 행동과 측정 지표로 재작성하라.
- 총평이 200자 미만 → 분석이 부족하다. 증거를 더 인용하라.
- 스크립트 실패 시 성찰 중단 → 직접 수집으로 대체하고 계속 진행하라.
- harness-mem 노드 저장 누락 → 성찰 결과가 다음 세션에 연결되지 않는다.
