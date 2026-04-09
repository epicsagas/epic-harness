# epic harness

**6 条命令。自动触发技能。自我进化。**

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

一个 Claude Code 插件，**用 6 条命令替代 30 多条命令**，根据当前操作**自动触发技能**，并从你的失败模式中**自动进化出新技能**。更少的记忆负担，每次按键更高的智能。

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness 功能特性" width="100%" />
</p>

## 架构：4 环模型

```
Ring 0 — 自动驾驶（钩子，不可见）
  会话恢复、自动格式化、安全护栏、观测日志

Ring 1 — 6 条命令（由你调用）
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — 自动技能（上下文触发）
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — 进化（自我改进）
  观测工具使用 → 分析失败 → 自动生成技能 → 门控 → 重新加载
```

## 安装

```bash
# Claude Code 插件市场
/plugin marketplace add epicsagas/epic-harness
/plugin install harness@epic

# 或手动安装
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rust 二进制（可选，钩子速度约快 4 倍）

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# 从 crates.io 安装
cargo install epic-harness
# 或使用 cargo-binstall（预编译二进制，更快）
cargo binstall epic-harness

# 从源码安装
cargo install --path .
```

钩子会自动检测该二进制文件。如果不存在，则回退到 Node.js。

## 命令

| 命令 | 功能 |
|---------|-------------|
| `/spec` | 定义要构建的内容 — 明确需求，输出规格说明 |
| `/go` | 开始构建 — 自动规划、TDD 子代理、并行执行 |
| `/check` | 验证 — 并行代码审查 + 安全审计 + 性能检查 |
| `/ship` | 发布 — PR、CI、合并 |
| `/team` | 设计项目专属的代理团队 |
| `/evolve` | 手动触发进化 / 查看状态 / 回滚 |

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

静默运行，无需用户操作。以**单一 Rust 二进制文件**（`epic-harness`）加子命令的形式实现，若二进制不可用则回退到 Node.js。

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| 钩子 | 触发时机 | 功能 |
|------|------|------|
| **resume** | 会话开始时 | 恢复上下文、加载记忆、检测技术栈 |
| **guard** | Bash 执行前 | 拦截 force-push-to-main、rm -rf /、DROP prod |
| **polish** | 编辑完成后 | 自动格式化（Biome/Prettier/ruff/gofmt）+ 类型检查 |
| **observe** | 每次工具使用 | 记录到 `.harness/obs/` 供进化使用 |
| **snapshot** | 压缩前 | 保存状态到 `.harness/sessions/` |
| **reflect** | 会话结束时 | 分析失败、生成进化技能、门控 |

## 评估系统（Ring 3 核心）

将 A-Evolve 的基准测试模式融入 Claude Code 的钩子系统。

### 多维评分

每次工具调用按 3 个维度评分。权重可通过 `src/ts/common.ts`（或 `src/hooks/common.rs`）中的 `SCORE_WEIGHTS` 配置：

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

所有阈值均为 `src/ts/common.ts`（或 `src/hooks/common.rs`）中的可配置常量：

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
    ↓ .harness/obs/session_{id}.jsonl
分析（SessionEnd）
    ↓ SessionAnalysis：按工具、按扩展名、分数分布
    ↓ 模式：repeated_same_error、fix_then_break、long_debug_loop、thrashing
生成（4 条路径：模式 / 弱工具 / 弱文件类型 / 高频错误）
    ↓ .harness/evolved/{skill}/SKILL.md
门控（格式检查、去重、上限 10 个、停滞检查）
    ↓ .harness/evolved_backup/（最佳检查点）
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

通过 `.harness/guard-rules.yaml` 添加项目专属的安全规则：

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

规则与内置防护（force-push-to-main、rm -rf /、DROP prod）合并生效。

## 跨项目学习

选择加入，在项目间共享失败模式：

```bash
touch .harness/.cross-project-enabled  # 选择加入
```

启用后：
- 会话结束时将匿名化的模式导出到 `~/.harness-global/patterns.jsonl`
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

## 项目数据（`.harness/`）

epic harness 会在你的项目中创建 `.harness/` 目录：

```
.harness/
├── memory/           # 项目模式和规则（持久化）
├── sessions/         # 会话快照（用于恢复）
├── obs/              # 工具使用观测日志（JSONL，按会话）
├── evolved/          # 自动进化的技能
├── evolved_backup/   # 最佳检查点（用于停滞回滚）
├── dispatch/         # 技能调度日志（JSONL）
├── team/             # /team 生成的代理和技能
├── evolution.jsonl   # 完整进化历史
├── metrics.json      # 聚合统计 + 技能归因
└── guard-rules.yaml  # 自定义防护规则（可选）
```

将 `.harness/` 加入 `.gitignore` 或提交到仓库 — 由你决定。

## 开发

### Rust（主要 — 约快 4 倍）

```bash
cargo install --path .          # 构建 + 安装到 ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # 更新插件二进制
```

### Node.js（备选）

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### 钩子调度方式

`hooks.json` 中的每个钩子按以下顺序查找 Rust 二进制文件，然后回退到 Node.js：

```
1. 插件本地: hooks/bin/epic-harness
2. PATH:     ~/.cargo/bin/epic-harness（通过 cargo install）
3. 回退:     node hooks/scripts/<hook>.js
```

### 测试

```bash
cargo test       # 98 个 Rust 单元测试
npm test         # Node.js 单元 + 端到端测试
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
