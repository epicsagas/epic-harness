# harness-mem MCP 서버 등록 가이드

`hooks/scripts/mem-mcp.cjs`는 JSON-RPC 2.0 over stdio MCP 서버입니다.
Claude Code, Cursor, Gemini CLI 등 MCP를 지원하는 모든 에이전트에서 네이티브 도구로 메모리를 쿼리·기록할 수 있습니다.

## Claude Code에서 harness-mem MCP 등록

`~/.claude/settings.json`에 아래 블록을 추가하세요 (경로는 실제 설치 위치로 교체):

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/epic-harness/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

또는 프로젝트 루트의 `.claude/settings.json`에 추가하면 프로젝트 범위로 적용됩니다.

## Cursor에서 등록

`.cursor/mcp.json` (또는 전역 `~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/epic-harness/hooks/scripts/mem-mcp.cjs"],
      "env": {
        "HARNESS_ROOT": "/path/to/.harness"
      }
    }
  }
}
```

## Gemini CLI에서 등록

`~/.gemini/settings.json`:

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/epic-harness/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

## 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `HARNESS_ROOT` | `~/.harness` | 메모리 저장소 루트 경로 |

## 제공 도구 (5개)

| 도구 | 설명 |
|------|------|
| `mem_add` | 새 메모리 노드 추가 (아키텍처 결정, 패턴, 오류 등) |
| `mem_query` | tag/type/project 필터로 노드 조회 |
| `mem_search` | 전체 텍스트 키워드 검색 |
| `mem_related` | 지식 그래프 엣지 BFS 탐색으로 연관 노드 반환 |
| `mem_context` | 세션 시작 시 프로젝트 컨텍스트 로드 |

## 자동 등록 (예정)

`epic-harness install --mcp` 플래그 실행 시 현재 에이전트를 자동 감지하여 위 설정을 자동으로 주입합니다.

## 테스트

```bash
# 단위 테스트 직접 실행
node hooks/scripts/mem-mcp.test.cjs

# stdio 수동 테스트
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test","version":"1.0"}}}' \
  | node hooks/scripts/mem-mcp.cjs

# tools/list 확인
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | node hooks/scripts/mem-mcp.cjs
```
