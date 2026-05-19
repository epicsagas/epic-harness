// i18n — Korean / English UI strings
// Add new keys here; use t('key') in templates.

type Lang = 'ko' | 'en';

export const translations = {
  // ── Common
  loading:          { ko: '로딩 중…',       en: 'Loading…' },
  loadError:        { ko: '데이터 로드 오류', en: 'Load error' },
  noData:           { ko: '데이터 없음',      en: 'No data' },
  reset:            { ko: '필터 초기화',      en: 'Reset' },
  noResults:        { ko: '필터 결과 없음',   en: 'No results' },
  copied:           { ko: '복사됨!',          en: 'Copied!' },
  errorPrefix:      { ko: '오류',             en: 'Error' },
  modeLabel:        { ko: '모드:',            en: 'Mode:' },

  // ── Common table headers
  colTool:          { ko: '툴',         en: 'Tool' },
  colCalls:         { ko: '호출 수',    en: 'Calls' },
  colAvgScore:      { ko: '평균 점수',  en: 'Avg Score' },
  colDate:          { ko: '날짜',       en: 'Date' },
  colTrend:         { ko: '추세',       en: 'Trend' },
  colStatus:        { ko: '상태',       en: 'Status' },
  colType:          { ko: '유형',       en: 'Type' },
  colValue:         { ko: '값',         en: 'Value' },
  colField:         { ko: '필드',       en: 'Field' },
  colDescription:   { ko: '설명',       en: 'Description' },
  colPattern:       { ko: '패턴',       en: 'Pattern' },
  colAgents:        { ko: '에이전트 수', en: 'Agents' },
  colBestFor:       { ko: '적합한 용도', en: 'Best For' },
  colSuccessRate:   { ko: '성공률',     en: 'Success Rate' },
  colSessionId:     { ko: '세션 ID',    en: 'Session ID' },
  colToolCalls:     { ko: '툴 호출 수', en: 'Tool Calls' },
  colFailures:      { ko: '실패 수',    en: 'Failures' },
  colSummaryPatterns: { ko: '요약 / 패턴', en: 'Summary / Patterns' },
  colObs:           { ko: '관찰 수',    en: 'Obs' },
  colSkills:        { ko: '스킬 수',    en: 'Skills' },
  colHook:          { ko: '훅',         en: 'Hook' },
  colCommand:       { ko: '커맨드',     en: 'Command' },
  colTrigger:       { ko: '트리거',     en: 'Trigger' },
  colEffect:        { ko: '효과',       en: 'Effect' },
  colPolishResult:  { ko: 'Polish 결과',       en: 'Polish result' },
  colFailureType:   { ko: '기록되는 실패 유형', en: 'Failure type recorded' },
  colPatternDetection: { ko: '패턴 감지',      en: 'Pattern detection' },
  colResource:      { ko: '리소스',     en: 'Resource' },
  colPurpose:       { ko: '목적',       en: 'Purpose' },
  colConstant:      { ko: '상수',       en: 'Constant' },
  colThreshold:     { ko: '임계값',     en: 'Threshold' },
  colId:            { ko: 'ID',         en: 'ID' },
  colProject:       { ko: '프로젝트',   en: 'Project' },
  colGoal:          { ko: '목표',       en: 'Goal' },
  colMode:          { ko: '모드',       en: 'Mode' },
  colStarted:       { ko: '시작 시각',  en: 'Started' },
  colDuration:      { ko: '소요 시간',  en: 'Duration' },

  // ── Dashboard
  pageDashboard:      { ko: '대시보드',   en: 'Dashboard' },
  pageDashboardDesc:  { ko: '4-Ring 아키텍처 상태 · 평가 점수 · 시스템 상태', en: '4-Ring architecture status · eval scores · system health' },
  ring0Desc:          { ko: '훅 6개 활성',         en: '6 hooks active' },
  ring1Desc:          { ko: '사용자 커맨드 10개',   en: '10 user commands' },
  ring2Desc:          { ko: '컨텍스트 트리거 15개', en: '15 context-triggered' },
  ring3Desc:          { ko: '관찰 → 진화 루프',     en: 'observe → evolve loop' },
  statSessions:       { ko: '세션',                 en: 'Sessions' },
  statSessionsSub:    { ko: '이 프로젝트',           en: 'this project' },
  statAvgScore:       { ko: '평균 점수',             en: 'Avg Score' },
  statAvgScoreSub:    { ko: '복합 점수 (성공 50% + 품질 30% + 비용 20%)', en: 'composite (success 50% + quality 30% + cost 20%)' },
  statTrend:          { ko: '추세',                  en: 'Trend' },
  statTrendSub:       { ko: '세션 대비 세션',         en: 'session-over-session' },
  statStagnation:     { ko: '정체 횟수',             en: 'Stagnation' },
  statStagnationSub:  { ko: '한도 3회 · 초과 시 자동 롤백', en: 'limit 3 · auto-rollback on exceed' },
  statTotalCalls:     { ko: '총 호출 수',            en: 'Total Calls' },
  statTotalCallsSub:  { ko: '최근 세션 합산',         en: 'recent sessions combined' },
  statFailures:       { ko: '실패 수',               en: 'Failures' },
  evalScoreTitle:     { ko: '평가 점수',              en: 'Eval Score' },
  toolStatsTitle:     { ko: '툴 통계 (상위 5개)',      en: 'Tool Stats (Top 5)' },
  recentActivityTitle:{ ko: '최근 활동',              en: 'Recent Activity' },
  sessionLabel:       { ko: '세션',                  en: 'Session' },
  callsAvgScore:      { ko: '호출, 평균 점수',        en: 'calls, avg score' },
  failuresLabel:      { ko: '실패',                  en: 'failures' },
  recentSessionNone:  { ko: '최근 세션 없음',         en: 'No recent sessions' },
  failuresSub:        { ko: 'recent_sessions 합산',  en: 'combined from recent sessions' },

  // ── Agents
  pageAgents:         { ko: '내부 에이전트',   en: 'Internal Agents' },
  pageAgentsDesc:     { ko: '/go 및 /check 페이즈에서 사용되는 4개의 내장 에이전트 · /team으로 확장 가능', en: '4 built-in agents used by /go and /check phases · extendable via /team' },
  activeAgents:       { ko: '현재 활성 에이전트',          en: 'Active Agents' },
  noActiveAgent:      { ko: '현재 작업 중인 에이전트 없음', en: 'No active agents' },
  lastTool:           { ko: '마지막 툴:',  en: 'last tool:' },
  scoreLabel:         { ko: '점수:',       en: 'score:' },
  lowSuccessTools:    { ko: '/team 패턴 — 주의 필요 툴 (success_rate < 85%)', en: '/team Patterns — Low Success Tools (success_rate < 85%)' },
  allToolsOk:         { ko: '모든 툴 정상 (success_rate ≥ 85%)', en: 'All tools healthy (success_rate ≥ 85%)' },
  statusCol:          { ko: '상태',        en: 'Status' },
  needsAttention:     { ko: '주의 필요',   en: 'Needs attention' },
  sessionActivity:    { ko: '세션별 에이전트 활동 요약', en: 'Session Activity Summary' },
  noRecentSession:    { ko: '최근 세션 없음', en: 'No recent sessions' },
  teamPatternsTitle:  { ko: '/team 오케스트레이션 패턴', en: '/team Orchestration Patterns' },
  patternPipelineDesc:   { ko: '순차 핸드오프, 빌드 → 테스트 → 리뷰',   en: 'Sequential handoffs, build → test → review' },
  patternFanOutDesc:     { ko: '병렬 독립 작업 후 머지',                 en: 'Parallel independent tasks, then merge' },
  patternExpertPoolDesc: { ko: '작업 유형별 전문가 라우팅',               en: 'Route to specialist per task type' },
  patternProducerReviewerDesc: { ko: '한 명 빌드, 한 명 리뷰, 반복',     en: 'One builds, one reviews, iterate' },
  patternSupervisorDesc: { ko: '중앙 코디네이터가 워커에 작업 배분',      en: 'Central coordinator dispatches to workers' },
  agentBuilderDesc:   { ko: 'TDD로 단일 작업 구현. 테스트 먼저 작성 후 코드 구현 및 검증. /go에서 개별 요구사항 작업 실행에 사용.', en: 'Implements a single task using TDD. Writes test first, then code, then verifies. Used by /go to execute individual requirement tasks.' },
  agentReviewerDesc:  { ko: '코드 품질, 정확성, 스타일, 테스트 커버리지 리뷰. /check 시 auditor 및 test runner와 병렬 실행.', en: 'Reviews code for quality, correctness, style, and test coverage. Launched by /check in parallel with auditor and test runner.' },
  agentAuditorDesc:   { ko: '보안 취약점 및 성능 이슈 감사. /check 시 reviewer와 병렬 실행.', en: 'Audits code for security vulnerabilities and performance issues. Parallel execution with reviewer during /check phase.' },
  agentPlannerDesc:   { ko: '목표를 순서가 있고 병렬화 가능한 의존성 포함 작업으로 분해. 승인된 spec에서 실행 계획 생성 시 /go에서 사용.', en: 'Breaks down a goal into ordered, parallelizable tasks with dependencies. Used by /go to create the execution plan from approved spec.' },

  // ── Commands
  pageCommands:       { ko: '커맨드',      en: 'Commands' },
  pageCommandsDesc:   { ko: '10개의 슬래시 커맨드 — 카드를 클릭하면 복사됩니다', en: '10 user-invoked slash commands — click any card to copy' },
  cmdDiscoverDesc:    { ko: '솔루션을 정하기 전에 문제를 탐색하고 정의', en: 'Explore and define the problem before specifying a solution' },
  cmdSpecDesc:        { ko: '코딩 전에 요구사항 정의', en: 'Define requirements before coding' },
  cmdGoDesc:          { ko: '자동 플랜 + TDD로 빌드', en: 'Build with auto-plan + TDD' },
  cmdCheckDesc:       { ko: '리뷰 + 보안 감사 + 테스트', en: 'Review + security audit + tests' },
  cmdShipDesc:        { ko: 'PR 생성, CI 검증, 머지', en: 'Create PR, verify CI, merge' },
  cmdEvolveDesc:      { ko: '스킬 진화 조회 및 트리거', en: 'Inspect or trigger skill evolution' },
  cmdTeamDesc:        { ko: '프로젝트별 에이전트 팀 생성', en: 'Generate project-specific agent team' },
  cmdOrbitDesc:       { ko: '자율 spec→ship 파이프라인', en: 'Autonomous spec→ship pipeline' },
  cmdGitCcDesc:       { ko: '자동 타입 선택 컨벤셔널 커밋', en: 'Conventional commit with auto type selection' },
  cmdGitDesc:         { ko: '멀티 레포 git 작업 (sync/bump/tags)', en: 'Cross-repo git operations (sync/bump/tags)' },

  // ── Skills
  pageSkills:         { ko: '자동 스킬',   en: 'Auto Skills' },
  pageSkillsDesc:     { ko: '15개 컨텍스트 트리거 스킬 + _dispatch 코어 라우터 · 카드를 클릭하면 복사됩니다', en: '15 context-triggered skills + _dispatch core router · click any card to copy' },
  skillDispatchDesc:  { ko: '컨텍스트에 따라 적합한 스킬로 자동 라우팅', en: 'Auto-routes tasks to the right skill based on context' },
  skillTddDesc:       { ko: '테스트 우선 개발 사이클', en: 'Test-first development cycle' },
  skillDebugDesc:     { ko: '체계적인 근본 원인 분석', en: 'Systematic root cause analysis' },
  skillSecureDesc:    { ko: '인증/DB/API 코드 보안 체크리스트', en: 'Security checklist for auth/db/api code' },
  skillVerifyDesc:    { ko: '완료 전 빌드 + 테스트 + 린트 확인', en: 'Build + test + lint before marking done' },
  skillSimplifyDesc:  { ko: '200줄 초과 파일에서 트리거', en: 'Triggered on files > 200 lines' },
  skillPerfDesc:      { ko: 'DB/API 코드 성능 분석', en: 'Performance analysis for DB/API code' },
  skillReviewDesc:    { ko: '코드 품질 및 로직 리뷰', en: 'Code quality and logic review' },
  skillRefactorDesc:  { ko: '안전한 구조 개선', en: 'Safe structural improvement' },
  skillMigrateDesc:   { ko: '데이터베이스 및 스키마 마이그레이션 안전성', en: 'Database and schema migration safety' },
  skillApiDesignDesc: { ko: 'REST/GraphQL API 설계 패턴', en: 'REST/GraphQL API design patterns' },
  skillDocDesc:       { ko: '문서 생성', en: 'Documentation generation' },
  skillTestGenDesc:   { ko: '미커버 코드 자동 테스트 생성', en: 'Auto test generation for uncovered code' },
  skillCiDesc:        { ko: 'CI/CD 파이프라인 설정', en: 'CI/CD pipeline configuration' },
  skillDeployDesc:    { ko: '안전한 배포 체크리스트', en: 'Safe deployment checklist' },

  // ── Evolution
  pageEvolve:         { ko: '평가 & 진화',   en: 'Eval & Evolve' },
  pageEvolveDesc:     { ko: '관찰 → 분석 → 진화 → 게이트 → 재로드 자기 개선 루프', en: 'Observe → Analyze → Evolve → Gate → Reload self-improvement loop' },
  loadingEvolution:   { ko: '진화 데이터 로딩 중…', en: 'Loading evolution data…' },
  statSessionsAnalyzed: { ko: '분석된 세션 수',  en: 'Sessions Analyzed' },
  statFailurePatterns:  { ko: '실패 패턴 수',    en: 'Failure Patterns' },
  statEvolvedSkills:    { ko: '진화된 스킬 수',  en: 'Evolved Skills' },
  statMaxSkillsCap:     { ko: '최대 스킬 한도',  en: 'Max Skills Cap' },
  evolvedSkillsTitle:   { ko: '진화된 스킬',     en: 'Evolved Skills' },
  evolutionHistoryTitle:{ ko: '진화 히스토리',   en: 'Evolution History' },
  trendImproving:     { ko: '↑ 개선 중',   en: '↑ improving' },
  trendDeclining:     { ko: '↓ 하락 중',   en: '↓ declining' },
  trendStable:        { ko: '→ 안정',      en: '→ stable' },
  seedingThresholdsTitle: { ko: '스킬 시딩 임계값', en: 'Skill Seeding Thresholds' },
  seedTypeWeakTool:       { ko: '취약 툴',         en: 'Weak tool' },
  seedTypeWeakFileType:   { ko: '취약 파일 유형',   en: 'Weak file type' },
  seedTypeHighFreqError:  { ko: '고빈도 오류',      en: 'High-freq error' },
  seedTypeStagnationRollback: { ko: '정체 롤백',   en: 'Stagnation rollback' },
  seedThreshWeakTool:     { ko: '성공률 < 0.6, 최소 관찰 5회', en: 'success_rate < 0.6, min 5 observations' },
  seedThreshWeakExt:      { ko: '성공률 < 0.5, 최소 관찰 3회', en: 'success_rate < 0.5, min 3 observations' },
  seedThreshHighFreq:     { ko: '5회 이상 발생',    en: '5+ occurrences' },
  seedThreshStagnation:   { ko: '5% 개선 없이 3세션 연속', en: '3 sessions without 5% improvement' },
  noEvolvedSkills:    { ko: '진화된 스킬 없음 — 더 많은 세션이 쌓이면 자동 생성됩니다', en: 'No evolved skills yet — they will be generated automatically as sessions accumulate' },
  filterCountFmt:     { ko: (n: number, total: number) => `${n}건 / 전체 ${total}건`, en: (n: number, total: number) => `${n} / ${total} records` },
  allTrend:           { ko: '전체 추세',     en: 'All Trends' },
  patternSearch:      { ko: '패턴 / 요약 검색…', en: 'Search Pattern / Summary…' },
  noHistoryFilter:    { ko: '필터 결과 없음',   en: 'No results' },
  noHistoryEmpty:     { ko: '아직 진화 히스토리가 없습니다', en: 'No evolution history yet' },

  // ── Hooks
  pageHooks:          { ko: '훅',           en: 'Hooks' },
  pageHooksDesc:      { ko: '자동 파일럿 자동화를 제공하는 Claude Code 훅 6개 · Rust 단일 바이너리', en: '6 Claude Code hooks providing autopilot automation · Rust single binary' },
  hookRegistryTitle:  { ko: '훅 레지스트리', en: 'Hook Registry' },
  hookResumeEffect:   { ko: '세션 복원 + 진화된 스킬 로드', en: 'Restore session + load evolved skills' },
  hookGuardEffect:    { ko: '위험한 셸 패턴 차단',          en: 'Block dangerous shell patterns' },
  hookObserveEffect:  { ko: '3축 점수를 obs JSONL에 기록',  en: 'Record 3-axis scores to obs JSONL' },
  hookPolishEffect:   { ko: '자동 포맷 + 타입 체크',        en: 'Auto-format + typecheck' },
  hookSnapshotEffect: { ko: 'sessions/에 세션 상태 저장',   en: 'Save session state to sessions/' },
  hookReflectEffect:  { ko: '스킬 진화 + 메트릭 저장',      en: 'Evolve skills + save metrics' },
  hookFlowTitle:      { ko: '훅 흐름도',     en: 'Hook Flow' },
  guardRulesTitle:    { ko: '가드 규칙 확장', en: 'Guard Rules Extension' },
  guardRulesDesc:     { ko: '프로젝트 루트의 .harness/guard-rules.yaml 파일로 커스텀 차단/경고 규칙을 추가하세요.', en: 'Add custom block/warn rules via .harness/guard-rules.yaml in your project root.' },
  polishFeedbackTitle: { ko: 'Polish → Observe 피드백', en: 'Polish → Observe Feedback' },
  polishFeedbackDesc:  { ko: 'Polish 훅 결과가 observe 파이프라인에 자동으로 기록됩니다.', en: 'Polish hook results auto-record into the observe pipeline.' },
  polishFormatFail:    { ko: '포맷 실패',    en: 'Format failure' },
  polishTypecheckFail: { ko: '타입 체크 실패', en: 'Typecheck failure' },
  polishFormatFeedDesc:     { ko: 'repeated_same_error 감지기에 데이터 제공', en: 'Feeds repeated_same_error detector' },
  polishTypecheckFeedDesc:  { ko: 'fix_then_break 감지기에 데이터 제공',     en: 'Feeds fix_then_break detector' },
  onSessionStart:     { ko: '세션 시작 시',       en: 'On session start' },
  onPreTool:          { ko: '모든 툴 호출 전',    en: 'Before every tool call' },
  onPostTool:         { ko: '툴 호출 후 (async)', en: 'After tool call (async)' },
  onPostEdit:         { ko: '파일 편집 후',       en: 'After file edit' },
  onPreCompact:       { ko: '컨텍스트 압축 전',   en: 'Before context compact' },
  onSessionEnd:       { ko: '세션 종료 시',       en: 'On session end' },

  // ── Integrations
  pageIntegrations:       { ko: '통합',       en: 'Integrations' },
  pageIntegrationsDesc:   { ko: 'AI 코딩 툴 6개 통합 · 설정 파일은 integrations/ 폴더', en: '6 AI coding tool integrations · configs in integrations/' },
  loadingIntegrations:    { ko: '통합 상태 로딩 중…', en: 'Loading integration status…' },
  statusInstalled:        { ko: '설치됨',     en: 'Installed' },
  statusNotInstalled:     { ko: '미설치',     en: 'Not installed' },
  sharedResourcesTitle:   { ko: '공유 리소스', en: 'Shared Resources' },
  rowCommands:            { ko: '커맨드',     en: 'Commands' },
  rowSkills:              { ko: '스킬',       en: 'Skills' },
  rowAgents:              { ko: '에이전트',   en: 'Agents' },
  rowHooks:               { ko: '훅',         en: 'Hooks' },

  // ── Memory
  pageMemory:             { ko: 'harness-mem',   en: 'harness-mem' },
  loadingGraph:           { ko: '지식 그래프 로딩 중…', en: 'Loading knowledge graph…' },
  mcpToolsTitle:          { ko: 'MCP 툴',          en: 'MCP Tools' },
  smartRecallScoringTitle: { ko: '스마트 검색 점수 산식', en: 'Smart Recall Scoring' },
  memRecallDesc:          { ko: '스마트 문맥 검색 — 힌트 + 프로젝트 + 그래프 이웃', en: 'Smart contextual recall — hint + project + graph neighbors' },
  memAddDesc:             { ko: '유형별 자동 중요도로 노드 추가',         en: 'Add node with auto-importance by type' },
  memSearchDesc:          { ko: 'FTS5 키워드 검색, 중요도 순 결과',       en: 'FTS5 keyword search, results ranked by importance' },
  memQueryDesc:           { ko: '고급 필터링을 위한 SQL 수준 쿼리',       en: 'SQL-level query for advanced filtering' },
  memContextDesc:         { ko: '프로젝트 범위 스마트 검색 (힌트 없음)',   en: 'Project-scoped smart recall (no hint)' },
  memRelatedDesc:         { ko: '노드 ID 기준 BFS 그래프 탐색',           en: 'BFS graph traversal from a node ID' },
  memNoData:              { ko: 'harness-mem 데이터 없음', en: 'No harness-mem data' },
  memWip:                 { ko: 'harness-mem은 현재 WIP 상태입니다.\nMCP 서버(harness-mem)를 통해 노드가 추가되면 그래프가 표시됩니다.',
                            en: 'harness-mem is currently WIP.\nThe graph will appear once nodes are added via the MCP server (harness-mem).' },

  // ── Orbit Pipeline
  pageOrbitDesc:          { ko: 'spec → go → check → ship → evolve — 단일 커맨드로 spec에서 PR까지 실행', en: 'spec → go → check → ship → evolve — single-command spec-to-PR execution' },
  entryModeTitle:         { ko: '진입 모드',   en: 'Entry Mode' },
  runningPipelines:       { ko: '실행 중인 파이프라인', en: 'Running Pipelines' },
  pipelineHistoryTitle:   { ko: '파이프라인 히스토리', en: 'Pipeline History' },
  pipelineStateSchemaTitle: { ko: '파이프라인 상태 스키마', en: 'Pipeline State Schema' },
  safetyMechanismsTitle:  { ko: '안전 메커니즘', en: 'Safety Mechanisms' },
  safetyGuardTitle:       { ko: '동시 실행 가드',  en: 'Concurrent orbit guard' },
  safetyDeadlineTitle:    { ko: '마감 시간 강제',  en: 'Deadline enforcement' },
  safetyCrashTitle:       { ko: '충돌 복구',        en: 'Crash recovery' },
  safetyWorktreeTitle:    { ko: '워크트리 안전성',  en: 'Worktree safety' },
  noGoal:                 { ko: '(goal 없음)',      en: '(no goal)' },
  startedAt:              { ko: '시작',             en: 'Started' },
  deadlineAt:             { ko: '마감',             en: 'Deadline' },
  expired:                { ko: '만료',             en: 'Expired' },
  minutesLeft:            { ko: (n: number) => `${n}분 남음`, en: (n: number) => `${n}m left` },
  checkFails:             { ko: '체크 실패',         en: 'Check fails' },
  filterCountOrbit:       { ko: (n: number, total: number) => `${n}건 / 전체 ${total}건`, en: (n: number, total: number) => `${n} / ${total}` },
  searchGoal:             { ko: '목표 검색…',        en: 'Search goal…' },
  searchProject:          { ko: '프로젝트 검색…',    en: 'Search project…' },
  allStatus:              { ko: '전체 상태',          en: 'All statuses' },
  noPipelines:            { ko: '실행된 파이프라인 없음', en: 'No pipelines found' },
  durationMin:            { ko: (n: number) => `${n}분`, en: (n: number) => `${n}m` },
  safetyGuard:            { ko: '파이프라인 동시 실행 1개 제한', en: 'Single concurrent pipeline limit' },
  safetyDeadline:         { ko: '30분 하드 타임아웃',           en: '30-minute hard timeout' },
  safetyCrash:            { ko: '45분 stale 임계값, phase_history 기준 복구', en: '45-minute stale threshold, recovery from phase_history' },
  safetyWorktree:         { ko: '격리 빌드, worktree 손실 시 state 보존', en: 'Isolated build, state preserved on worktree loss' },
  interactiveDesc:        { ko: '/discover → /spec 수동 실행 후 orbit 트리거. 요구사항이 불명확할 때.', en: 'Run /discover → /spec manually, then trigger orbit. Use when requirements are unclear.' },
  councilDesc:            { ko: '4-voice 병렬 자동 spec (Architect·Critic·Implementor·QA). 복잡한 요구사항.', en: '4-voice parallel auto-spec (Architect·Critic·Implementor·QA). For complex requirements.' },
  directDesc:             { ko: 'auto-spec 후 즉시 빌드 시작. 요구사항이 명확할 때.', en: 'Auto-spec then build immediately. Use when requirements are clear.' },

  // ── Settings
  pageSettings:           { ko: '설정',       en: 'Settings' },
  pageSettingsDesc:       { ko: 'epic-harness 평가 가중치, 진화 튜닝, 시스템 정보', en: 'epic-harness eval weights, evolution tuning, and system info' },
  loadingMetrics:         { ko: '메트릭 로딩 중…', en: 'Loading metrics…' },
  evalWeightsTitle:       { ko: '평가 점수 가중치', en: 'Eval Score Weights' },
  evolutionTuningTitle:   { ko: '진화 튜닝 상수',  en: 'Evolution Tuning Constants' },
  systemInfoTitle:        { ko: '시스템 정보',      en: 'System Info' },
  labelSessionsAnalyzed:  { ko: '분석된 세션 수',  en: 'Sessions analyzed' },
  labelCurrentTrend:      { ko: '현재 추세',        en: 'Current trend' },
  labelStagnationCount:   { ko: '정체 횟수',        en: 'Stagnation count' },
  labelVersion:           { ko: '버전',             en: 'Version' },
  dangerZoneTitle:        { ko: '위험 구역',         en: 'Danger Zone' },
  resetEvolutionLabel:    { ko: '진화 초기화',       en: 'Reset Evolution' },
  resetEvolutionDesc:     { ko: '모든 진화된 스킬을 삭제합니다', en: 'Removes all evolved skills' },
  clearMetricsLabel:      { ko: '메트릭 초기화',    en: 'Clear Metrics' },
  clearMetricsDesc:       { ko: '점수 히스토리를 초기화합니다', en: 'Resets score history' },
} as const;

type TranslationKey = keyof typeof translations;
type TranslationValue = typeof translations[TranslationKey];

import { writable, derived, get } from 'svelte/store';

// Detect browser language on first load; fall back to 'en'
function detectLang(): Lang {
  const nav = typeof navigator !== 'undefined' ? navigator.language : '';
  return nav.startsWith('ko') ? 'ko' : 'en';
}

export const lang = writable<Lang>(detectLang());
export function setLang(l: Lang) { lang.set(l); }
export function getLang(): Lang { return get(lang); }

// translate helper — pure function, lang-aware
function translate(l: Lang, key: TranslationKey, args: number[]): string {
  const entry = translations[key];
  const val = (entry[l] ?? entry['en']) as string | ((...a: number[]) => string);
  return typeof val === 'function' ? val(...args) : val;
}

// tStore — reactive translate function. Use `$tStore` in Svelte templates.
// `$tStore('key')` re-evaluates whenever lang changes.
export const tStore = derived(lang, ($l) => (key: TranslationKey, ...args: number[]) => translate($l, key, args));

// t() — non-reactive convenience for use inside TS functions (e.g. deadlineRemaining).
export function t(key: TranslationKey, ...args: number[]): string {
  return translate(get(lang), key, args);
}
