# epic harness

**7 条命令。自动触发技能。自我进化。**

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

一个 Claude Code 插件，**用 7 条命令替代 30 多条命令**，根据当前操作**自动触发技能**，并从你的失败模式中**自动进化出新技能**。更少的记忆负担，每次按键更高的智能。

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness 功能特性" width="100%" />
</p>

## 架构：4 环模型

```
Ring 0 — 自动驾驶（钩子，不可见）
  会话恢复、自动格式化、安全护栏、观测日志

Ring 1 — 7 条命令（由你调用）
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — 自动技能（上下文触发）
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — 进化（自我改进）
  观测工具使用 → 分析失败 → 自动生成技能 → 门控 → 重新加载
```

## 安装

```
# Claude Code 插件（推荐）
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# 或从源码安装
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### 从二进制安装

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# 从 crates.io 安装
cargo install epic-harness

# 预编译二进制（更快，无需编译）
cargo binstall epic-harness

# 从源码安装
cargo install --path .
```

钩子会自动检测该二进制文件。如果不存在，钩子会回退到 Node.js。

## 多工具支持

epic-harness 支持 Claude Code 以及另外 6 款 AI 编程工具。所有工具共享同一个 `~/.harness/projects/{slug}/` 数据目录。

| 工具 | Ring 0 钩子 | 命令/提示词 | 技能 | 代理 |
|------|-------------|------------|------|------|
| **Claude Code** | ✓ 完整 | ✓ 7 条命令 | ✓ 11 个技能 | ✓ 4 |
| **Codex CLI** | ✓ 完整¹ | ✓ 7 个提示词 | ✓ 7（`~/.agents/skills/`） | ✓ 4 |
| **Gemini CLI** | ✓ 部分² | ✓ 7 条命令 | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ 完整³ | ✓ 7 条命令 | ✓ 通过规则 | ✓ 4 |
| **OpenCode** | ✓ 部分⁴ | ✓ 7 条命令 | — | ✓ 4 |
| **Cline** | ✓ 完整⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ 需在 `~/.codex/config.toml` 中设置 `codex_hooks = true`；PostToolUse 仅拦截 Bash
² 无 `PreToolUse` 等效项 — guard 在 `BeforeModel` 级别运行
³ 需要 Cursor 1.7+
⁴ JS 插件：`session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel 钩子脚本
⁶ 无钩子系统 — 约定通过 `.aider/CONVENTIONS.md` + `.aider.conf.yml` 注入

### 为其他工具安装

```bash
# 交互式菜单（选择要安装的工具）
epic install

# 直接安装
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/（需要 Cursor 1.7+）
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# 安装到项目本地
epic install cursor --local

# 预览（不做实际更改）
epic install gemini --dry-run
```

工具目录中的集成文件（`hooks.json`、命令、代理、技能、规则等）会从二进制文件**同步**：缺失或过时的文件会被写入。`GEMINI.md` 和 `AGENTS.md` 仅在不存在时创建。

## 统一记忆

所有代理共享存储在 `~/.harness/memory.db`（SQLite + FTS5）中的单一知识图谱。无需 Node.js 或外部运行时。

### 智能召回

内存检索使用**复合评分**而非简单转储最新 N 条记录：

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **重要性**按节点类型自动设置：decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **访问追踪**：频繁召回的记忆会自然浮到顶部
- **渐进衰减**：未使用的记忆会随时间降低重要性（每 30 天 10%，最低 0.05）
- **图谱增强**：召回跟随 1 跳边来呈现相关上下文

### CLI

```bash
# 智能召回 — 为当前任务按相关性排序
epic mem recall "auth refactor" --project my-project

# 添加记忆节点（重要性按类型自动设置，或显式指定）
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# 过滤查询（包含重要性 + 访问次数）
epic mem query --type decision --project my-project

# 全文搜索（按重要性排序）
epic mem search "JWT"

# 智能上下文（重要性加权，而非仅最新）
epic mem context --project my-project

# 知识图谱 Web UI
epic mem serve          # → http://localhost:7700

# 在 Claude Code 中注册为 MCP 服务器（无需 Node.js）
epic mem mcp-install

# 将所有节点导出为 Markdown 供 Git 备份
epic mem export --out ./docs/memory
```

### MCP 工具（6 个）

注册为 MCP 服务器（`epic mem mcp-install`）后，代理可以直接调用这些工具：

| 工具 | 用途 |
|------|---------|
| `mem_recall` | **主要。** 带提示 + 项目 + 图谱邻居的智能上下文召回 |
| `mem_add` | 按类型添加自动重要性节点（或显式 0.0–1.0） |
| `mem_search` | FTS5 关键词搜索，按重要性排序 |
| `mem_query` | 按标签/类型/项目过滤 |
| `mem_context` | 项目范围智能召回（无提示） |
| `mem_related` | 从节点 ID 进行 BFS 图谱遍历 |

### 知识图谱的工作原理

图谱从正常的会话工作中自动积累——无需手动输入。

**数据流：**

```
PostToolUse hook → observe (3-axis scoring) → obs/*.jsonl
                                                   ↓
SessionEnd hook → reflect (pattern detection) → memory.db nodes + edges
                                                   ↓  （重要性按类型设置）
SessionStart hook → resume (smart recall) → 下次会话获得相关性排序提示
                              ↓
                    decay_importance() → 未使用节点逐渐淡出
```

**节点类型 (7)：**

| 类型 | 创建方式 | 默认重要性 |
|------|-----------|-------------------|
| `decision` | 手动 / MCP | 0.9 |
| `resolution` | 手动 / MCP | 0.8 |
| `concept` | 手动 / MCP | 0.7 |
| `project` | 手动 / MCP | 0.7 |
| `pattern` | 自动 (reflect) | 0.5 |
| `error` | 自动 (reflect) | 0.4 |
| `session` | 自动 (reflect) | 0.2 |

**记忆生命周期：**

| 事件 | 发生的事情 |
|-------|-------------|
| 通过搜索/召回/上下文召回节点 | `access_count++`，`accessed_at` 更新 |
| 30 天以上未访问 | 重要性衰减 10%（最低 0.05） |
| 180 天以上未访问 | 标记为 `stale`，从召回中排除 |
| 标记为 `pinned` 的节点 | 免于衰减 |

**自动积累条件：**

| 条件 | 创建的节点 |
|-----------|-------------|
| 每次会话结束 | `session`（始终） |
| 相同错误连续 ≥3 次 | `error` (repeated_same_error) |
| Edit→Error 交替出现 | `pattern` (thrashing) |
| 工具成功率 <60%（至少 5 次观测） | `pattern` (weak_tool) |
| 文件类型成功率 <50%（至少 3 次观测） | `pattern` (weak_filetype) |
| Edit 成功 → Bash 错误循环 | `pattern` (fix_then_break) |

> **注意：** 干净的会话（无错误）只会产生 `session` 节点。在经历 2–3 次包含构建失败、测试失败或调试循环的真实开发会话后，图谱会变得丰富。

现有的基于文件的记忆（`nodes/*.md`、`edges.jsonl`）在首次运行时会自动迁移到 SQLite。

## 命令

| 命令 | 功能 |
|---------|-------------|
| `/spec` | 定义要构建的内容 — 明确需求，输出规格说明 |
| `/go` | 开始构建 — 自动规划、TDD 子代理、并行执行 |
| `/check` | 验证 — 并行代码审查 + 安全审计 + 性能检查 |
| `/ship` | 发布 — PR、CI、合并 |
| `/team` | 跨项目创建和同步组织级代理团队 |
| `/evolve` | 手动触发进化 / 查看状态 / 回滚 |

## 团队 (`epic team`)

团队是**组织级别**的，不绑定到具体项目。在任何项目中运行 `/team` 都会丰富共享代理定义池——绝不会静默覆盖。

### 工作原理

```
epic team                      # 交互式：扫描项目 → 设计 → 写入 → 同步
         ↓
~/.harness/orgs/epic/teams/backend/   ← 全局存储（跨项目持久化）
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code 在会话开始时自动发现
├── domain-expert.md                  ← 角色定义 + 注入团队上下文
├── reviewer.md
└── tester.md
         ↓
下次会话：代理激活 — 由 Claude 自动选择或显式调用
```

### CLI 参考

```bash
# 创建或更新团队（交互式 4 阶段流程）
epic team

# 浏览
epic team list                        # 当前组织的所有团队
epic team list --org netflix          # 指定组织的团队
epic team show backend                # 配置、使命、代理
epic team show backend --playbook     # + 完整累积的剧本

# 分派到项目
epic team sync backend                # 分派：复制代理 → .claude/agents/backend/
epic team link backend                # 分派 + 在团队配置中注册项目

# 从项目召回
epic team delete backend              # 召回：仅从当前项目移除
epic team unlink backend              # delete 的别名

# 解散（从组织完全移除）
epic team delete backend --global     # 从组织存储 + 本地副本永久删除

# 历史
epic team history backend reviewer    # 列出代理的 .history/ 备份
```

### 在编程代理中使用团队

同步后，代理在下次会话中自动可用：

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert 实现支付网关
@reviewer 检查此 PR 的边界情况
@tester 为 auth 编写集成测试

# 或让代理根据任务上下文自动选择
```

每个代理文件携带同步时注入的**团队上下文**部分：

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

代理知道其团队、使命以及如何按需加载完整剧本——
而不会用它膨胀上下文窗口。

### 多组织

```bash
epic team                          # 在 "epic" 组织中积累（默认）
epic team --org netflix            # 单独的 Netflix 风格拓扑
epic team --org client-x           # 按客户划分的项目
```

同一组织中相同的团队名称 = 有意的跨项目共享。
`epic/teams/backend` 从每个创建或关联它的项目中积累知识。

### 团队类型

| 类型 | 关键词 | 默认代理 |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### 合并策略 — 无静默覆盖

| 对象 | 规则 |
|--------|------|
| 代理 — 新增 | 自动添加 |
| 代理 — 未变更 | 跳过 |
| 代理 — 已变更 | **提示**（默认：保留现有）。替换时 → 备份到 `.history/` |
| `playbook.md` | 始终**追加** — 从不截断 |
| `mission.md` — 已变更 | **提示**（默认：保留现有） |

## 自动技能（Ring 2）

技能根据上下文自动触发，无需手动调用。

| 技能 | 触发条件 |
|-------|--------------|
| **tdd** | 实现新功能时 |
| **debug** | 测试失败或出现错误时 |
| **secure** | 涉及认证/数据库/API/密钥代码时 |
| **perf** | 涉及循环、查询、渲染代码时 |
| **simplify** | 文件超过 200 行或复杂度过高时 |
| **document** | 新增或修改公共 API 时 |
| **verify** | 完成 /go 或 /ship 之前 |
| **context** | 上下文窗口使用超过 70% 时 |

## 钩子（Ring 0）

静默运行，无需用户操作。以**单一 Rust 二进制文件**（`epic-harness`）加子命令的形式实现。如果二进制文件不存在，钩子会回退到 Node.js。

```
epic resume | guard | polish | observe | snapshot | reflect
```

| 钩子 | 触发时机 | 功能 |
|------|------|------|
| **resume** | 会话开始时 | 恢复上下文、加载记忆、检测技术栈 |
| **guard** | Bash 执行前 | 拦截 force-push-to-main、rm -rf /、DROP prod |
| **polish** | 编辑完成后 | 自动格式化（Biome/Prettier/ruff/gofmt）+ 类型检查 |
| **observe** | 每次工具使用 | 记录到 `~/.harness/projects/{slug}/obs/` 供进化使用 |
| **snapshot** | 压缩前 | 保存状态到 `~/.harness/projects/{slug}/sessions/` |
| **reflect** | 会话结束时 | 分析失败、生成进化技能、门控 |

## 评估系统（Ring 3 核心）

将 A-Evolve 的基准测试模式融入 Claude Code 的钩子系统。

### 多维评分

每次工具调用按 3 个维度评分。权重可通过 `~/.harness/config.toml`中的 `SCORE_WEIGHTS` 配置：

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (默认: 0.5)                          (默认: 0.3)                             (默认: 0.2)
```

| 维度 | 衡量内容 | 各工具标准 |
|-----------|-----------------|-------------------|
| `tool_success` | 是否成功？（0/1） | 9 类失败分类 |
| `output_quality` | 输出质量信号（0.0-1.0） | Bash：警告、空输出。Edit：重复编辑检测 |
| `execution_cost` | 效率代理指标（0.0-1.0） | 输出大小、静默成功命令白名单 |

### 失败分类（9 类）

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### 模式检测（4 种类型）

所有阈值均为 `~/.harness/config.toml`中的可配置常量：

| 模式 | 检测内容 | 常量 | 默认值 |
|---------|---------|----------|---------|
| `repeated_same_error` | 连续 N 次以上相同错误 | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | 编辑成功 → 构建/测试失败 | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | 同一文件连续 N 次以上操作 | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | 同一文件上编辑↔错误交替出现 | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### 技能生成阈值

| 触发条件 | 常量 | 默认值 |
|---------|----------|---------|
| 弱工具（低成功率） | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| 弱文件类型 | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| 高频错误 | `HIGH_FREQ_ERROR_MIN` | 5 |

### 停滞门控

- `STAGNATION_LIMIT`（默认：3）个会话无改善 → 自动回滚进化技能到最佳检查点
- `IMPROVEMENT_THRESHOLD`（默认：5%）
- 趋势追踪：通过线性回归判断 `improving` / `stable` / `declining`
- 发生冲突时，静态技能始终优先于进化技能

### 进化流程

```
观测（PostToolUse — 3 维评分）
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
分析（SessionEnd）
    ↓ SessionAnalysis：按工具、按扩展名、分数分布
    ↓ 模式：repeated_same_error、fix_then_break、long_debug_loop、thrashing
生成（4 条路径：模式 / 弱工具 / 弱文件类型 / 高频错误）
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
门控（格式检查、去重、上限 10 个、停滞检查）
    ↓ ~/.harness/projects/{slug}/evolved_backup/（最佳检查点）
重新加载（下次会话 — resume.ts 报告指标 + 加载进化技能）
```

```bash
/evolve              # 立即运行进化
/evolve status       # 仪表盘：分数、趋势、模式、技能
/evolve history      # 长期分析：完整历史、技能效果、调度统计
/evolve cross-project # 跨项目模式分析
/evolve rollback     # 恢复到之前的最佳状态
/evolve reset        # 清除所有进化数据
```

## 冷启动预设

无需等待 5 个会话才能获得有用的进化技能。首次会话时，epic harness 会检测你的技术栈并自动应用预设技能：

| 技术栈 | 预设技能 |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`、`evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

预设只是补充 — 随着数据积累，它们会被真正的进化技能所替代。

## 并发会话安全

每个会话写入独立的观测文件（`session_{date}_{pid}_{random}.jsonl`）。同一项目上的多个 Claude Code 会话不会互相破坏数据。reflect 钩子会合并当天所有的会话文件进行分析。

## 自定义防护规则

通过项目根目录的 `.harness/guard-rules.yaml` 添加项目专属的安全规则：

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

规则与内置防护（force-push-to-main、rm -rf /、DROP prod）合并生效。将此文件纳入 git 可与团队共享安全规则。

## 跨项目学习

选择加入，在项目间共享失败模式：

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # 选择加入
```

启用后：
- 会话结束时将匿名化的模式导出到 `~/.harness/global_patterns.jsonl`
- 会话开始时显示来自其他项目薄弱环节的提示
- 使用 `/evolve cross-project` 查看聚合模式

## 技能效果追踪

每个进化技能都通过 A/B 归因分数进行追踪：

```
/evolve history → 技能效果部分

| 技能               | 会话数   | 启用时分数 | 未启用时分数  | 差异   |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

正向差异 = 技能有效。负向差异 = 考虑通过 `/evolve rollback` 移除。

## Polish → Observe 反馈

polish 钩子（自动格式化 + 类型检查）的结果会回馈到观测管道：

- 格式化失败 → 记录为 `lint_fail`
- TypeScript 错误 → 记录为 `build_fail`
- 成功 → 记录完整分数

这意味着即使错误来自 polish 钩子而非手动命令，"编辑 → 类型错误 → 编辑 → 类型错误"的反复模式也能被检测到。

## 项目数据（`~/.harness/projects/{slug}/`）

项目专属数据存储在你的主目录中。项目删除后仍然保留，且不会污染 git 历史。

```
~/.harness/projects/{slug}/
├── memory/           # 项目模式和规则（持久化）
├── sessions/         # 会话快照（用于恢复）
├── obs/              # 工具使用观测日志（JSONL，按会话）
├── evolved/          # 自动进化的技能
├── evolved_backup/   # 最佳检查点（用于停滞回滚）
├── dispatch/         # 技能调度日志（JSONL）
├── team/             # legacy（已由 ~/.harness/orgs/ 取代）
├── evolution.jsonl   # 完整进化历史
└── metrics.json      # 聚合统计 + 技能归因

~/.harness/
├── memory.db         # SQLite 知识图谱（nodes + edges + FTS5）
├── graph.json        # 缓存的图谱（供 Web UI 使用）
└── orgs/             # epic team 全局存储
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

你仍然可以在项目根目录使用 `.harness/guard-rules.yaml` 与团队共享安全规则。

## 开发

### 构建

```bash
cargo install --path .          # 构建 + 安装到 ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # 更新插件二进制
```

### 钩子调度方式

`hooks.json` 中的每个钩子按以下顺序查找 Rust 二进制文件：

```
1. 插件本地: hooks/bin/epic-harness
2. PATH:     ~/.cargo/bin/epic-harness（通过 cargo install）
```

### 测试

```bash
cargo test       # Rust 单元 + 集成测试
```

## 致谢

epic harness 受到以下项目的启发并基于其理念构建：

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — 自动进化与基准测试模式
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code 代理技能系统
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — 全面的 Claude Code 模式
- [gstack](https://github.com/garrytan/gstack) — 插件架构参考
- [harness](https://github.com/revfactory/harness) — 钩子与线束基础设施模式
- [serena](https://github.com/oraios/serena) — 自主代理设计
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — 多命令框架架构
- [superpowers](https://github.com/obra/superpowers) — Claude Code 扩展模式

## 许可证

[Apache 2.0](LICENSE)
