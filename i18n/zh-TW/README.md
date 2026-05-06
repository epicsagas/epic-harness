# epic harness

**6 個指令。自動觸發技能。自我進化。**

<p align="center">
<a href="../../README.md">English</a> | <a href="../ja/README.md">日本語</a> | <a href="../ko/README.md">한국어</a> | <a href="../de/README.md">Deutsch</a> | <a href="../fr/README.md">Français</a> | <a href="../zh-CN/README.md">简体中文</a> | <a href="../zh-TW/README.md">繁體中文</a> | <a href="../pt-BR/README.md">Português</a> | <a href="../es/README.md">Español</a> | <a href="../hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache_2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/Version-0.2.5-brightgreen.svg" alt="Version">
  <img src="https://img.shields.io/badge/Claude_Code-Plugin-purple.svg" alt="Claude Code Plugin">
  <img src="https://img.shields.io/badge/Architecture-4_Ring-orange.svg" alt="4-Ring Architecture">
  <img src="https://img.shields.io/badge/Mode-Self_Evolving-green.svg" alt="Self Evolving">
  <a href="https://buymeacoffee.com/epicsaga"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

一個 Claude Code 外掛，**用 6 個指令取代 30 多個**，根據你正在做的事情**自動觸發技能**，並從你自身的失敗模式中**進化出新技能**。更少的記憶負擔，每次按鍵都更聰明。

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness features" width="100%" />
</p>

## 架構：4 環模型

```
Ring 0 — 自動駕駛（hooks，不可見）
  工作階段恢復、自動格式化、安全護欄、觀測記錄

Ring 1 — 6 個指令（由你呼叫）
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — 自動技能（依情境觸發）
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — 進化（自我改善）
  觀測工具使用 → 分析失敗 → 自動生成技能 → 品質閘門 → 重新載入
```

## 安裝

```
# Claude Code 外掛（推薦）
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# 或從原始碼安裝
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### 從二進位檔安裝

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# 從 crates.io
cargo install epic-harness

# 預建構二進位（更快，無需編譯）
cargo binstall epic-harness

# 從原始碼
cargo install --path .
```

hooks 會自動偵測二進位檔。若不存在，hooks 會退回到 Node.js。

## 多工具支援

epic-harness 支援 Claude Code 以及另外 6 款 AI 程式設計工具。所有工具共享同一個 `~/.harness/projects/{slug}/` 資料目錄。

| 工具 | Ring 0 Hooks | 指令/提示詞 | 技能 | 代理 |
|------|-------------|------------|------|------|
| **Claude Code** | ✓ 完整 | ✓ 6 個指令 | ✓ 10 個技能 | ✓ 4 |
| **Codex CLI** | ✓ 完整¹ | ✓ 6 個提示詞 | ✓ 7（`~/.agents/skills/`） | ✓ 4 |
| **Gemini CLI** | ✓ 部分² | ✓ 6 個指令 | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ 完整³ | ✓ 6 個指令 | ✓ 透過規則 | ✓ 4 |
| **OpenCode** | ✓ 部分⁴ | ✓ 6 個指令 | — | ✓ 4 |
| **Cline** | ✓ 完整⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ 需在 `~/.codex/config.toml` 中設定 `codex_hooks = true`；PostToolUse 僅攔截 Bash
² 無 `PreToolUse` 等效項 — guard 在 `BeforeModel` 層級執行
³ 需要 Cursor 1.7+
⁴ JS 外掛：`session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel hook 腳本
⁶ 無 hook 系統 — 慣例透過 `.aider/CONVENTIONS.md` + `.aider.conf.yml` 注入

### 為其他工具安裝

```bash
# 互動式選單（選擇要安裝的工具）
epic install

# 直接安裝
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/（需要 Cursor 1.7+）
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# 安裝至專案本地
epic install cursor --local

# 預覽（不進行任何變更）
epic install gemini --dry-run
```

工具目錄中的整合檔案（`hooks.json`、指令、代理、技能、規則等）會從二進位檔**同步**：缺失或過時的檔案會被寫入。`GEMINI.md` 和 `AGENTS.md` 僅在不存在時建立。

## 統一記憶

所有代理共享儲存在 `~/.harness/memory.db`（SQLite + FTS5）中的單一知識圖譜。無需 Node.js 或外部執行環境。

### 智慧召回

記憶擷取使用**複合評分**而非單純轉儲最新 N 筆記錄：

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **重要性**按節點類型自動設定：decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **存取追蹤**：頻繁召回的記憶會自然浮至頂部
- **漸進衰減**：未使用的記憶會隨時間降低重要性（每 30 天 10%，最低 0.05）
- **圖譜增強**：召回跟隨 1 跳邊來呈現相關上下文

### CLI

```bash
# 智慧召回 — 為當前任務按相關性排序
epic mem recall "auth refactor" --project my-project

# 新增記憶節點（重要性按類型自動設定，或明確指定）
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# 篩選查詢（包含重要性 + 存取次數）
epic mem query --type decision --project my-project

# 全文搜尋（按重要性排序）
epic mem search "JWT"

# 智慧上下文（重要性加權，而非僅最新）
epic mem context --project my-project

# 知識圖譜 Web UI
epic mem serve          # → http://localhost:7700

# 在 Claude Code 中註冊為 MCP 伺服器（無需 Node.js）
epic mem mcp-install

# 將所有節點匯出為 Markdown 供 Git 備份
epic mem export --out ./docs/memory
```

### MCP 工具（6 個）

註冊為 MCP 伺服器（`epic mem mcp-install`）後，代理可以直接呼叫這些工具：

| 工具 | 用途 |
|------|---------|
| `mem_recall` | **主要。** 帶提示 + 專案 + 圖譜鄰居的智慧上下文召回 |
| `mem_add` | 按類型新增自動重要性節點（或明確 0.0–1.0） |
| `mem_search` | FTS5 關鍵詞搜尋，按重要性排序 |
| `mem_query` | 按標籤/類型/專案篩選 |
| `mem_context` | 專案範圍智慧召回（無提示） |
| `mem_related` | 從節點 ID 進行 BFS 圖譜遍歷 |

### 知識圖譜的運作方式

圖譜從正常的工作階段中自動累積——無需手動輸入。

**資料流：**

```
PostToolUse hook → observe (3-axis scoring) → obs/*.jsonl
                                                   ↓
SessionEnd hook → reflect (pattern detection) → memory.db nodes + edges
                                                   ↓  （重要性按類型設定）
SessionStart hook → resume (smart recall) → 下次工作階段獲得相關性排序提示
                              ↓
                    decay_importance() → 未使用節點逐漸淡出
```

**節點類型 (7)：**

| 類型 | 建立方式 | 預設重要性 |
|------|-----------|-------------------|
| `decision` | 手動 / MCP | 0.9 |
| `resolution` | 手動 / MCP | 0.8 |
| `concept` | 手動 / MCP | 0.7 |
| `project` | 手動 / MCP | 0.7 |
| `pattern` | 自動 (reflect) | 0.5 |
| `error` | 自動 (reflect) | 0.4 |
| `session` | 自動 (reflect) | 0.2 |

**記憶生命週期：**

| 事件 | 發生的事情 |
|-------|-------------|
| 透過搜尋/召回/上下文召回節點 | `access_count++`，`accessed_at` 更新 |
| 30 天以上未存取 | 重要性衰減 10%（最低 0.05） |
| 180 天以上未存取 | 標記為 `stale`，從召回中排除 |
| 標記為 `pinned` 的節點 | 免於衰減 |

**自動累積條件：**

| 條件 | 建立的節點 |
|-----------|-------------|
| 每次工作階段結束 | `session`（始終） |
| 相同錯誤連續 ≥3 次 | `error` (repeated_same_error) |
| Edit→Error 交替出現 | `pattern` (thrashing) |
| 工具成功率 <60%（至少 5 次觀測） | `pattern` (weak_tool) |
| 檔案類型成功率 <50%（至少 3 次觀測） | `pattern` (weak_filetype) |
| Edit 成功 → Bash 錯誤循環 | `pattern` (fix_then_break) |

> **注意：** 乾淨的工作階段（無錯誤）只會產生 `session` 節點。在經歷 2–3 次包含建置失敗、測試失敗或除錯循環的實際開發工作階段後，圖譜會變得豐富。

現有的基於檔案的記憶（`nodes/*.md`、`edges.jsonl`）在首次執行時會自動遷移至 SQLite。

## 指令

| 指令 | 功能說明 |
|---------|-------------|
| `/spec` | 定義要建構的內容 — 釐清需求、產出規格 |
| `/go` | 開始建構 — 自動規劃、TDD 子代理、平行執行 |
| `/check` | 驗證 — 平行執行程式碼審查 + 安全稽核 + 效能檢測 |
| `/ship` | 交付 — PR、CI、合併 |
| `/team` | 跨專案建立並同步組織級代理團隊 |
| `/evolve` | 手動觸發進化 / 狀態 / 回滾 |

## 團隊 (`epic team`)

團隊是**組織級別**的，不綁定到特定專案。在任何專案中執行 `/team` 都會豐富共享代理定義池——絕不會靜默覆寫。

### 運作方式

```
epic team                      # 互動式：掃描專案 → 設計 → 寫入 → 同步
         ↓
~/.harness/orgs/epic/teams/backend/   ← 全局存儲（跨專案持久化）
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code 在工作階段開始時自動發現
├── domain-expert.md                  ← 角色定義 + 注入團隊上下文
├── reviewer.md
└── tester.md
         ↓
下次工作階段：代理啟動 — 由 Claude 自動選擇或明確呼叫
```

### CLI 參考

```bash
# 建立或更新團隊（互動式 4 階段流程）
epic team

# 瀏覽
epic team list                        # 當前組織的所有團隊
epic team list --org netflix          # 指定組織的團隊
epic team show backend                # 設定、使命、代理
epic team show backend --playbook     # + 完整累積的劇本

# 分派至專案
epic team sync backend                # 分派：複製代理 → .claude/agents/backend/
epic team link backend                # 分派 + 在團隊設定中登錄專案

# 從專案召回
epic team delete backend              # 召回：僅從當前專案移除
epic team unlink backend              # delete 的別名

# 解散（從組織完全移除）
epic team delete backend --global     # 從組織存儲 + 本地副本永久刪除

# 歷史
epic team history backend reviewer    # 列出代理的 .history/ 備份
```

### 在程式設計代理中使用團隊

同步後，代理在下次工作階段中自動可用：

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert 實作付款閘道
@reviewer 檢查此 PR 的邊界情況
@tester 為 auth 撰寫整合測試

# 或讓代理根據任務上下文自動選擇
```

每個代理檔案攜帶同步時注入的**團隊上下文**區段：

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

代理知道其團隊、使命以及如何按需載入完整劇本——
而不會用它膨脹上下文視窗。

### 多組織

```bash
epic team                          # 在 "epic" 組織中累積（預設）
epic team --org netflix            # 獨立的 Netflix 風格拓撲
epic team --org client-x           # 按客戶劃分的專案
```

同一組織中相同的團隊名稱 = 有意的跨專案共享。
`epic/teams/backend` 從每個建立或連結它的專案中累積知識。

### 團隊類型

| 類型 | 關鍵詞 | 預設代理 |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### 合併策略 — 無靜默覆寫

| 物件 | 規則 |
|--------|------|
| 代理 — 新增 | 自動新增 |
| 代理 — 未變更 | 跳過 |
| 代理 — 已變更 | **提示**（預設：保留現有）。替換時 → 備份到 `.history/` |
| `playbook.md` | 始終**附加** — 從不截斷 |
| `mission.md` — 已變更 | **提示**（預設：保留現有） |

## 自動技能（Ring 2）

技能根據情境自動觸發，無需手動呼叫。

| 技能 | 觸發條件 |
|-------|--------------|
| **tdd** | 實作新功能時 |
| **debug** | 測試失敗或出現錯誤時 |
| **secure** | 觸及認證/資料庫/API/金鑰相關程式碼時 |
| **perf** | 涉及迴圈、查詢、渲染程式碼時 |
| **simplify** | 檔案超過 200 行或複雜度過高時 |
| **document** | 新增或變更公開 API 時 |
| **verify** | 完成 /go 或 /ship 之前 |
| **context** | 上下文視窗使用超過 70% 時 |

## Hooks（Ring 0）

不可見地運行，無需使用者操作。以**單一 Rust 二進位檔**（`epic-harness`）搭配子指令實作。若二進位檔不存在，hooks 會退回到 Node.js。

```
epic resume | guard | polish | observe | snapshot | reflect
```

| Hook | 時機 | 動作 |
|------|------|------|
| **resume** | 工作階段開始 | 恢復上下文、載入記憶、偵測技術堆疊 |
| **guard** | Bash 執行前 | 阻擋 force-push-to-main、rm -rf /、DROP prod |
| **polish** | 編輯後 | 自動格式化（Biome/Prettier/ruff/gofmt）+ 型別檢查 |
| **observe** | 每次工具使用 | 記錄至 `~/.harness/projects/{slug}/obs/` 供進化使用 |
| **snapshot** | 壓縮前 | 儲存狀態至 `~/.harness/projects/{slug}/sessions/` |
| **reflect** | 工作階段結束 | 分析失敗、播種進化技能、品質閘門 |

## 評估系統（Ring 3 核心）

將 A-Evolve 的基準測試模式融入 Claude Code 的 hook 系統。

### 多維度評分

每次工具呼叫依 3 個軸向評分。權重可透過 `~/.harness/config.toml`中的 `SCORE_WEIGHTS` 設定：

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (預設: 0.5)                          (預設: 0.3)                             (預設: 0.2)
```

| 維度 | 衡量指標 | 各工具準則 |
|-----------|-----------------|-------------------|
| `tool_success` | 是否成功？（0/1） | 9 類失敗分類 |
| `output_quality` | 輸出品質訊號（0.0-1.0） | Bash：警告、空輸出。Edit：重新編輯偵測 |
| `execution_cost` | 效率指標（0.0-1.0） | 輸出大小、靜默成功指令白名單 |

### 失敗分類（9 類）

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### 模式偵測（4 種類型）

所有閾值皆為 `~/.harness/config.toml`中的可設定常數：

| 模式 | 偵測內容 | 常數 | 預設值 |
|---------|---------|----------|---------|
| `repeated_same_error` | 相同錯誤連續出現 N 次以上 | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | 編輯成功 → 建構/測試失敗 | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | 對同一檔案持續操作 N 次以上 | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | 同一檔案上 Edit↔Error 交替出現 | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### 技能播種閾值

| 觸發條件 | 常數 | 預設值 |
|---------|----------|---------|
| 弱工具（低成功率） | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| 弱檔案類型 | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| 高頻錯誤 | `HIGH_FREQ_ERROR_MIN` | 5 |

### 停滯閘門

- `STAGNATION_LIMIT`（預設：3）個工作階段無改善 → 自動將進化技能回滾至最佳檢查點
- `IMPROVEMENT_THRESHOLD`（預設：5%）
- 趨勢追蹤：透過線性回歸分為 `improving` / `stable` / `declining`
- 發生衝突時，靜態技能始終優先於進化技能

### 進化流程

```
觀測（PostToolUse — 3 軸評分）
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
分析（SessionEnd）
    ↓ SessionAnalysis：逐工具、逐副檔名、分數分布
    ↓ 模式：repeated_same_error、fix_then_break、long_debug_loop、thrashing
播種（4 條路徑：模式 / 弱工具 / 弱檔案類型 / 高頻錯誤）
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
閘門（格式檢查、去重、上限 10 個、停滯檢查）
    ↓ ~/.harness/projects/{slug}/evolved_backup/（最佳檢查點）
重新載入（下次工作階段 — resume.ts 報告指標 + 載入進化技能）
```

```bash
/evolve              # 立即執行進化
/evolve status       # 儀表板：分數、趨勢、模式、技能
/evolve history      # 長期分析：完整歷史、技能效果、分派統計
/evolve cross-project # 跨專案模式分析
/evolve rollback     # 恢復至先前最佳狀態
/evolve reset        # 清除所有進化資料
```

## 冷啟動預設

無需等待 5 個工作階段才能獲得有用的進化技能。首次工作階段時，epic harness 會偵測你的技術堆疊並自動套用預設技能：

| 技術堆疊 | 預設技能 |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`、`evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

預設為補充性質 — 隨著資料累積，它們會被真正的進化技能取代。

## 並行工作階段安全性

每個工作階段寫入自己的觀測檔案（`session_{date}_{pid}_{random}.jsonl`）。同一專案上的多個 Claude Code 工作階段不會互相破壞資料。reflect hook 會合併同一天的所有檔案進行分析。

## 自訂安全規則

透過專案根目錄的 `.harness/guard-rules.yaml` 新增專案專屬的安全規則：

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

規則會與內建護欄（force-push-to-main、rm -rf /、DROP prod）合併。將此檔案納入 git 可與團隊共享安全規則。

## 跨專案學習

選擇加入以跨專案分享失敗模式：

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # 選擇加入
```

啟用後：
- 工作階段結束時，匿名化的模式匯出至 `~/.harness/global_patterns.jsonl`
- 工作階段開始時，顯示來自其他專案弱點區域的提示
- 使用 `/evolve cross-project` 查看彙總模式

## 技能效果追蹤

每個進化技能都透過 A/B 歸因分數進行追蹤：

```
/evolve history → 技能效果區段

| Skill              | Sessions | Score With | Score Without | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

正向差異 = 技能有幫助。負向差異 = 考慮透過 `/evolve rollback` 移除。

## Polish → Observe 回饋

polish hook（自動格式化 + 型別檢查）會將結果回饋至觀測管線：

- 格式化失敗 → 記錄為 `lint_fail`
- TypeScript 錯誤 → 記錄為 `build_fail`
- 成功 → 記錄完整分數

這意味著「編輯 → 型別錯誤 → 編輯 → 型別錯誤」的反覆模式即使錯誤來自 polish hook 而非手動指令，也能被偵測到。

## 專案資料（`~/.harness/projects/{slug}/`）

專案專屬資料存放在你的主目錄中。專案刪除後仍然保留，且不會污染 git 歷史。

```
~/.harness/projects/{slug}/
├── memory/           # 專案模式與規則（持久化）
├── sessions/         # 工作階段快照（供恢復使用）
├── obs/              # 工具使用觀測記錄（JSONL，逐工作階段）
├── evolved/          # 自動進化的技能
├── evolved_backup/   # 最佳檢查點（供停滯回滾使用）
├── dispatch/         # 技能分派記錄（JSONL）
├── team/             # legacy（已由 ~/.harness/orgs/ 取代）
├── evolution.jsonl   # 完整進化歷史
└── metrics.json      # 彙總統計 + 技能歸因

~/.harness/
├── memory.db         # SQLite 知識圖譜（nodes + edges + FTS5）
├── graph.json        # 快取的圖譜（供 Web UI 使用）
└── orgs/             # epic team 全局存儲
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

你仍然可以在專案根目錄使用 `.harness/guard-rules.yaml` 與團隊共享安全規則。

## 開發

### 建構

```bash
cargo install --path .          # 建構 + 安裝至 ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # 更新外掛二進位檔
```

### Hooks 如何分派

`hooks.json` 中的每個 hook 會在兩個位置尋找 Rust 二進位檔：

```
1. 外掛本地：hooks/bin/epic-harness
2. PATH：    ~/.cargo/bin/epic-harness（透過 cargo install）
```

### 測試

```bash
cargo test       # Rust 單元 + 整合測試
```

## 致謝

epic harness 的靈感來自以下專案並建立於其概念之上：

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — 自動化進化與基準測試模式
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code 代理技能系統
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — 全面的 Claude Code 模式
- [gstack](https://github.com/garrytan/gstack) — 外掛架構參考
- [harness](https://github.com/revfactory/harness) — Hook 與 harness 基礎架構模式
- [serena](https://github.com/oraios/serena) — 自主代理設計
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — 多指令框架架構
- [superpowers](https://github.com/obra/superpowers) — Claude Code 擴充模式

## 授權條款

[Apache 2.0](LICENSE)
