# epic harness

**6つのコマンド。自動トリガースキル。自己進化。**

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

Claude Codeプラグインで、**30以上のコマンドを6つに集約**し、作業内容に応じて**スキルを自動トリガー**し、失敗パターンから**新しいスキルを自己進化**させます。覚えるべきコマンドが少なく、キーストロークあたりのインテリジェンスが向上します。

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness features" width="100%" />
</p>

## アーキテクチャ：4リングモデル

```
Ring 0 — オートパイロット（フック、不可視）
  セッション復元、自動フォーマット、ガードレール、観測ログ

Ring 1 — 6つのコマンド（ユーザーが呼び出す）
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — 自動スキル（コンテキストトリガー）
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — 進化（自己改善）
  ツール使用を観測 → 失敗を分析 → スキルを自動生成 → ゲート → リロード
```

## インストール

```
# Claude Code プラグイン（推奨）
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# またはソースから
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### バイナリからインストール

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# crates.ioから
cargo install epic-harness

# ビルド済みバイナリ（高速、コンパイル不要）
cargo binstall epic-harness

# ソースから
cargo install --path .
```

バイナリはフックによって自動検出されます。存在しない場合はNode.jsにフォールバックします。

## マルチツールサポート

epic-harnessはClaude Codeと6つの追加AIコーディングツールで動作します。すべてのツールは同じ `~/.harness/projects/{slug}/` データディレクトリを共有します。

| ツール | Ring 0 フック | コマンド/プロンプト | スキル | エージェント |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ フル | ✓ 6コマンド | ✓ 10スキル | ✓ 4 |
| **Codex CLI** | ✓ フル¹ | ✓ 6プロンプト | ✓ 7（`~/.agents/skills/`） | ✓ 4 |
| **Gemini CLI** | ✓ 部分²  | ✓ 6コマンド | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ フル³ | ✓ 6コマンド | ✓ ルール経由 | ✓ 4 |
| **OpenCode** | ✓ 部分⁴ | ✓ 6コマンド | — | ✓ 4 |
| **Cline** | ✓ フル⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ `~/.codex/config.toml` に `codex_hooks = true` が必要; PostToolUseはBashのみインターセプト
² `PreToolUse` 相当機能なし — guardは `BeforeModel` レベルで実行
³ Cursor 1.7+ が必要
⁴ JSプラグイン: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel フックスクリプト
⁶ フックシステムなし — コンベンションを `.aider/CONVENTIONS.md` + `.aider.conf.yml` で注入

### 他のツールにインストール

```bash
# インタラクティブメニュー（インストールするツールを選択）
epic install

# 直接インストール
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/（Cursor 1.7+ 必要）
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# プロジェクトローカルにインストール
epic install cursor --local

# 変更なしでプレビュー
epic install gemini --dry-run
```

ツールディレクトリの統合ファイル（`hooks.json`、コマンド、エージェント、スキル、ルールなど）はバイナリから**同期**されます：不足または古いファイルが書き込まれます。`GEMINI.md` および `AGENTS.md` は存在しない場合のみ作成されます。

## 統合メモリ

すべてのエージェントは `~/.harness/memory.db`（SQLite + FTS5）に保存された単一のナレッジグラフを共有します。Node.jsや外部ランタイムは不要です。

### スマートリコール

メモリ取得は最新N件のダンプではなく**複合スコアリング**を使用します：

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **重要度** ノードタイプ別に自動設定：decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **アクセス追跡**：頻繁にリコールされるメモリは自然に上位に浮かぶ
- **緩やかな減衰**：未使用のメモリは時間とともに重要度が低下（30日ごとに10%、最低0.05）
- **グラフ拡張**：リコールは1ホップのエッジを辿り関連コンテキストを表面化

### CLI

```bash
# スマートリコール — 現在のタスクに関連性でランク付け
epic mem recall "auth refactor" --project my-project

# メモリノードを追加（重要度はタイプ別に自動設定、または明示的に指定）
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# フィルタークエリ（重要度 + アクセス数を含む）
epic mem query --type decision --project my-project

# 全文検索（重要度順にランク付け）
epic mem search "JWT"

# スマートコンテキスト（重要度加重、最新順でない）
epic mem context --project my-project

# ナレッジグラフのWeb UI
epic mem serve          # → http://localhost:7700

# Claude CodeにMCPサーバーとして登録（Node.js不要）
epic mem mcp-install

# すべてのノードをMarkdownにエクスポート（Gitバックアップ用）
epic mem export --out ./docs/memory
```

### MCPツール（6個）

MCPサーバーとして登録（`epic mem mcp-install`）すると、エージェントがこれらのツールを直接呼び出せます：

| ツール | 目的 |
|------|---------|
| `mem_recall` | **主要。** ヒント + プロジェクト + グラフ近傍を使ったスマートコンテキストリコール |
| `mem_add` | タイプ別自動重要度でノードを追加（または明示的に0.0–1.0） |
| `mem_search` | FTS5キーワード検索、重要度順にランク付け |
| `mem_query` | タグ/タイプ/プロジェクトでフィルタリング |
| `mem_context` | プロジェクトスコープのスマートリコール（ヒントなし） |
| `mem_related` | ノードIDからのBFSグラフ探索 |

### ナレッジグラフの仕組み

グラフは通常のセッション作業から自動的に蓄積されます — 手動入力は不要です。

**データフロー:**

```
PostToolUse hook → observe (3-axis scoring) → obs/*.jsonl
                                                   ↓
SessionEnd hook → reflect (pattern detection) → memory.db nodes + edges
                                                   ↓  （重要度はタイプ別に設定）
SessionStart hook → resume (smart recall) → 次のセッションに関連性ランク付きヒントを提供
                              ↓
                    decay_importance() → 未使用ノードは徐々にフェード
```

**ノードタイプ (7):**

| タイプ | 作成元 | デフォルト重要度 |
|------|-----------|-------------------|
| `decision` | 手動 / MCP | 0.9 |
| `resolution` | 手動 / MCP | 0.8 |
| `concept` | 手動 / MCP | 0.7 |
| `project` | 手動 / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**メモリライフサイクル:**

| イベント | 発生すること |
|-------|-------------|
| 検索/リコール/コンテキストでノードをリコール | `access_count++`, `accessed_at` 更新 |
| 30日以上アクセスなし | 重要度を10%減衰（最低0.05） |
| 180日以上アクセスなし | `stale` タグ、リコールから除外 |
| `pinned` タグのノード | 減衰免疫 |

**自動蓄積の条件:**

| 条件 | 作成されるノード |
|-----------|-------------|
| 各セッション終了時 | `session` (常時) |
| 同一エラーが3回以上連続 | `error` (repeated_same_error) |
| Edit→Errorの交互発生 | `pattern` (thrashing) |
| ツール成功率 <60% (最低5回の観測) | `pattern` (weak_tool) |
| ファイルタイプ成功率 <50% (最低3回の観測) | `pattern` (weak_filetype) |
| Edit成功 → Bashエラーのサイクル | `pattern` (fix_then_break) |

> **注意:** クリーンなセッション (エラーなし) は `session` ノードのみを生成します。グラフはビルド失敗、テスト失敗、デバッグサイクルを含む2〜3回の実際の開発セッション後に充実します。

既存のファイルベースのメモリ (`nodes/*.md`, `edges.jsonl`) は初回実行時に自動的にSQLiteへ移行されます。

## コマンド

| コマンド | 機能 |
|---------|-------------|
| `/spec` | 構築対象の定義 — 要件を明確化し、仕様書を生成 |
| `/go` | 構築実行 — 自動計画、TDDサブエージェント、並列実行 |
| `/check` | 検証 — コードレビュー + セキュリティ監査 + パフォーマンスチェックを並列実行 |
| `/ship` | リリース — PR作成、CI実行、マージ |
| `/team` | プロジェクト横断のorg-levelエージェントチームを作成・同期 |
| `/evolve` | 手動進化トリガー / ステータス確認 / ロールバック |

## チーム (`epic team`)

チームは**org-level**であり、プロジェクトに依存しません。任意のプロジェクトで `/team` を実行すると、共有エージェント定義のプールが豊かになります — 決してサイレントに上書きしません。

### 動作の仕組み

```
epic team                      # インタラクティブ: プロジェクトスキャン → 設計 → 書き込み → 同期
         ↓
~/.harness/orgs/epic/teams/backend/   ← グローバルストア（プロジェクト間で永続）
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Codeがセッション開始時に自動検出
├── domain-expert.md                  ← ロール定義 + チームコンテキスト注入
├── reviewer.md
└── tester.md
         ↓
次のセッション: エージェントがアクティブ — Claudeが自動選択または明示的に呼び出し
```

### CLIリファレンス

```bash
# チームを作成または更新（インタラクティブ4フェーズフロー）
epic team

# ブラウズ
epic team list                        # 現在のorgの全チーム
epic team list --org netflix          # 指定orgのチーム
epic team show backend                # 設定、ミッション、エージェント
epic team show backend --playbook     # + 完全な蓄積プレイブック

# プロジェクトにディスパッチ
epic team sync backend                # ディスパッチ: エージェントをコピー → .claude/agents/backend/
epic team link backend                # ディスパッチ + チーム設定にプロジェクトを登録

# プロジェクトからリコール
epic team delete backend              # リコール: 現在のプロジェクトからのみ削除
epic team unlink backend              # deleteの別名

# 解散（orgから完全に削除）
epic team delete backend --global     # orgストア + ローカルコピーを永久削除

# 履歴
epic team history backend reviewer    # エージェントの.history/バックアップを一覧
```

### コーディングエージェントからのチーム使用

同期後、次のセッションからエージェントが自動的に利用可能になります：

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert 決済ゲートウェイを実装してください
@reviewer このPRのエッジケースを確認してください
@tester authの統合テストを書いてください

# またはエージェントがタスクコンテキストに基づいて自動選択
```

各エージェントファイルには同期時に注入された**チームコンテキスト**セクションが含まれます：

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

エージェントはチーム、ミッション、完全なプレイブックをオンデマンドでロードする方法を知っています —
コンテキストウィンドウを膨らませることなく。

### マルチorg

```bash
epic team                          # "epic" orgに蓄積（デフォルト）
epic team --org netflix            # 別のNetflixスタイルトポロジー
epic team --org client-x           # クライアント別エンゲージメント
```

同じorgの同じチーム名 = 意図的なクロスプロジェクト共有。
`epic/teams/backend` はそれを作成またはリンクするすべてのプロジェクトから知識を蓄積します。

### チームタイプ

| タイプ | キーワード | デフォルトエージェント |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### マージ戦略 — サイレント上書きなし

| オブジェクト | ルール |
|--------|------|
| エージェント — 新規 | 自動追加 |
| エージェント — 変更なし | スキップ |
| エージェント — 変更あり | **プロンプト**（デフォルト: 既存を維持）。置換時 → `.history/` にバックアップ |
| `playbook.md` | 常に**追記** — 切り詰めなし |
| `mission.md` — 変更あり | **プロンプト**（デフォルト: 既存を維持） |

## 自動スキル（Ring 2）

スキルはコンテキストに基づいて自動的にトリガーされます。手動で呼び出す必要はありません。

| スキル | トリガー条件 |
|-------|--------------|
| **tdd** | 新機能の実装時 |
| **debug** | テスト失敗またはエラー発生時 |
| **secure** | 認証/DB/API/シークレット関連コードの変更時 |
| **perf** | ループ、クエリ、レンダリングコードの処理時 |
| **simplify** | ファイルが200行超またはの高複雑度の場合 |
| **document** | パブリックAPIの追加または変更時 |
| **verify** | /go または /ship の完了前 |
| **context** | コンテキストウィンドウの使用率が70%超の場合 |

## フック（Ring 0）

不可視で実行されます。ユーザーの操作は不要です。**単一のRustバイナリ**（`epic-harness`）のサブコマンドとして実装されており、バイナリがない場合はNode.jsにフォールバックします。

```
epic resume | guard | polish | observe | snapshot | reflect
```

| フック | タイミング | 動作 |
|------|------|------|
| **resume** | セッション開始時 | コンテキスト復元、メモリ読み込み、スタック検出 |
| **guard** | Bash実行前 | mainへのforce-push、rm -rf /、本番DBのDROPをブロック |
| **polish** | 編集後 | 自動フォーマット（Biome/Prettier/ruff/gofmt）+ 型チェック |
| **observe** | 全ツール使用時 | `~/.harness/projects/{slug}/obs/` にログ記録（進化用） |
| **snapshot** | コンパクト前 | `~/.harness/projects/{slug}/sessions/` に状態を保存 |
| **reflect** | セッション終了時 | 失敗を分析、進化スキルをシード、ゲート |

## 評価システム（Ring 3コア）

A-EvolveのベンチマークパターンをClaude Codeのフックシステムに統合します。

### 多次元スコアリング

すべてのツール呼び出しは3つの軸でスコアリングされます。重みは `~/.harness/config.toml`の `SCORE_WEIGHTS` で設定可能です：

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (デフォルト: 0.5)                       (デフォルト: 0.3)                          (デフォルト: 0.2)
```

| 次元 | 測定内容 | ツール別基準 |
|-----------|-----------------|-------------------|
| `tool_success` | 成功したか？（0/1） | 9カテゴリの失敗分類 |
| `output_quality` | 出力品質シグナル（0.0-1.0） | Bash: 警告、空出力。Edit: 再編集検出 |
| `execution_cost` | 効率性の指標（0.0-1.0） | 出力サイズ、サイレント成功コマンドのホワイトリスト |

### 失敗分類（9カテゴリ）

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### パターン検出（4タイプ）

すべての閾値は `~/.harness/config.toml`の設定可能な定数です：

| パターン | 検出内容 | 定数 | デフォルト |
|---------|---------|----------|---------|
| `repeated_same_error` | 同一エラーがN回以上連続 | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | 編集成功 → ビルド/テスト失敗 | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | 同一ファイルでN回以上の操作が停滞 | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | 同一ファイルで編集↔エラーが交互に発生 | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### スキルシーディング閾値

| トリガー | 定数 | デフォルト |
|---------|----------|---------|
| 弱いツール（低成功率） | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| 弱いファイルタイプ | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| 高頻度エラー | `HIGH_FREQ_ERROR_MIN` | 5 |

### 停滞ゲーティング

- `STAGNATION_LIMIT`（デフォルト: 3）セッション連続で改善なし → 進化スキルを最良チェックポイントに自動ロールバック
- `IMPROVEMENT_THRESHOLD`（デフォルト: 5%）
- トレンド追跡：線形回帰による `improving` / `stable` / `declining` 判定
- 競合時は静的スキルが進化スキルより常に優先

### 進化フロー

```
Observe（PostToolUse — 3軸スコアリング）
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze（SessionEnd）
    ↓ SessionAnalysis: ツール別、拡張子別、スコア分布
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed（4経路: パターン / 弱いツール / 弱いファイルタイプ / 高頻度エラー）
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate（フォーマットチェック、重複排除、上限10、停滞チェック）
    ↓ ~/.harness/projects/{slug}/evolved_backup/（最良チェックポイント）
Reload（次セッション — resume.tsがメトリクスを報告 + 進化スキルを読み込み）
```

```bash
/evolve              # 今すぐ進化を実行
/evolve status       # ダッシュボード: スコア、トレンド、パターン、スキル
/evolve history      # 長期分析: 全履歴、スキル効果、ディスパッチ統計
/evolve cross-project # クロスプロジェクトパターン分析
/evolve rollback     # 前回の最良状態を復元
/evolve reset        # すべての進化データをクリア
```

## コールドスタートプリセット

有用な進化スキルのために5セッション待つ必要はありません。初回セッション時に、epic harnessがスタックを検出し、プリセットスキルを自動適用します：

| スタック | プリセットスキル |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

プリセットは補助的なものであり、データが蓄積されると実際の進化スキルに置き換えられます。

## 並行セッション安全性

各セッションは固有の観測ファイル（`session_{date}_{pid}_{random}.jsonl`）に書き込みます。同一プロジェクトでの複数のClaude Codeセッションが互いのデータを破損することはありません。reflectフックは分析のために同日のすべてのファイルをマージします。

## カスタムガードルール

プロジェクトルートの `.harness/guard-rules.yaml` でプロジェクト固有の安全ルールを追加できます：

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

ルールは組み込みガード（mainへのforce-push、rm -rf /、本番DBのDROP）とマージされます。このファイルをgitに含めることでチームと安全ルールを共有できます。

## クロスプロジェクト学習

プロジェクト間で失敗パターンを共有するオプトイン機能：

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # オプトイン
```

有効化すると：
- セッション終了時に匿名化されたパターンを `~/.harness/global_patterns.jsonl` にエクスポート
- セッション開始時に他プロジェクトの弱点からのヒントを表示
- `/evolve cross-project` で集約パターンを確認可能

## スキル効果追跡

すべての進化スキルはA/B帰属スコアで追跡されます：

```
/evolve history → スキル効果セクション

| Skill              | Sessions | Score With | Score Without | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

正のデルタ = スキルが有効。負のデルタ = `/evolve rollback` での削除を検討。

## Polish → Observe フィードバック

polishフック（自動フォーマット + 型チェック）の結果は観測パイプラインにフィードバックされます：

- フォーマット失敗 → `lint_fail` として記録
- TypeScriptエラー → `build_fail` として記録
- 成功 → 完全なスコアで記録

これにより、polishフックからのエラーであっても、「編集 → 型エラー → 編集 → 型エラー」のスラッシングパターンが検出されます。

## プロジェクトデータ（`~/.harness/projects/{slug}/`）

プロジェクト固有のデータはホームディレクトリに保存されます。プロジェクト削除後も残り、gitの履歴を汚染しません。

```
~/.harness/projects/{slug}/
├── memory/           # プロジェクトパターンとルール（永続）
├── sessions/         # セッションスナップショット（復元用）
├── obs/              # ツール使用観測ログ（JSONL、セッション別）
├── evolved/          # 自動進化スキル
├── evolved_backup/   # 最良チェックポイント（停滞ロールバック用）
├── dispatch/         # スキルディスパッチログ（JSONL）
├── team/             # legacy (superseded by ~/.harness/orgs/)
├── evolution.jsonl   # 完全な進化履歴
└── metrics.json      # 集約統計 + スキル帰属

~/.harness/
├── memory.db         # SQLiteナレッジグラフ (nodes + edges + FTS5)
├── graph.json        # キャッシュされたグラフ (Web UI用)
└── orgs/             # epic team グローバルストア
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

プロジェクトルートの `.harness/guard-rules.yaml` でチームと安全ルールを共有することもできます。

## 開発

### ビルド

```bash
cargo install --path .          # ビルド + ~/.cargo/bin/ にインストール
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # プラグインバイナリを更新
```

### フックのディスパッチ方法

`hooks.json` の各フックは2箇所でRustバイナリを探します：

```
1. プラグインローカル: hooks/bin/epic-harness
2. PATH:              ~/.cargo/bin/epic-harness（cargo install経由）
```

### テスト

```bash
cargo test       # Rustユニット + 統合テスト
```

## 謝辞

epic harnessは以下のプロジェクトのアイデアにインスパイアされ、それらを基に構築されました：

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — 自動進化とベンチマークパターン
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Codeエージェントスキルシステム
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — 包括的なClaude Codeパターン
- [gstack](https://github.com/garrytan/gstack) — プラグインアーキテクチャのリファレンス
- [harness](https://github.com/revfactory/harness) — フックとハーネスのインフラパターン
- [serena](https://github.com/oraios/serena) — 自律エージェント設計
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — マルチコマンドフレームワークアーキテクチャ
- [superpowers](https://github.com/obra/superpowers) — Claude Code拡張パターン

## ライセンス

[Apache 2.0](LICENSE)
