# epic harness

**6つのコマンド。自動トリガースキル。自己進化。**

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

```bash
# Claude Code プラグインCLI
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# または手動で
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rustバイナリ（オプション、フックが約4倍高速）

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# crates.ioから
cargo install epic-harness
# またはcargo-binstallで（ビルド済み、より高速）
cargo binstall epic-harness

# ソースから
cargo install --path .
```

バイナリはフックによって自動検出されます。存在しない場合、フックはNode.jsにフォールバックします。

### 他のツールにインストール

まずRustバイナリをインストールし（必須）、次にツール用のインテグレーションをインストールします：

```bash
# インテグレーションをインストール（グローバル、デフォルト）
epic-harness install codex        # Codex CLI  → ~/.codex/
epic-harness install gemini       # Gemini CLI → ~/.gemini/
epic-harness install cursor       # Cursor     → ~/.cursor/
epic-harness install antigravity  # Antigravity → ~/.agents/ + AGENTS.md

# プロジェクトローカルにインストール
epic-harness install cursor --local

# 変更なしでプレビュー
epic-harness install gemini --dry-run
```

## コマンド

| コマンド | 機能 |
|---------|-------------|
| `/spec` | 構築対象の定義 — 要件を明確化し、仕様書を生成 |
| `/go` | 構築実行 — 自動計画、TDDサブエージェント、並列実行 |
| `/check` | 検証 — コードレビュー + セキュリティ監査 + パフォーマンスチェックを並列実行 |
| `/ship` | リリース — PR作成、CI実行、マージ |
| `/team` | プロジェクト固有のエージェントチームを設計 |
| `/evolve` | 手動進化トリガー / ステータス確認 / ロールバック |

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

不可視で実行されます。ユーザーの操作は不要です。**単一のRustバイナリ**（`epic-harness`）のサブコマンドとして実装されており、バイナリが利用できない場合はNode.jsにフォールバックします。

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| フック | タイミング | 動作 |
|------|------|------|
| **resume** | セッション開始時 | コンテキスト復元、メモリ読み込み、スタック検出 |
| **guard** | Bash実行前 | mainへのforce-push、rm -rf /、本番DBのDROPをブロック |
| **polish** | 編集後 | 自動フォーマット（Biome/Prettier/ruff/gofmt）+ 型チェック |
| **observe** | 全ツール使用時 | `.harness/obs/` にログ記録（進化用） |
| **snapshot** | コンパクト前 | `.harness/sessions/` に状態を保存 |
| **reflect** | セッション終了時 | 失敗を分析、進化スキルをシード、ゲート |

## 評価システム（Ring 3コア）

A-EvolveのベンチマークパターンをClaude Codeのフックシステムに統合します。

### 多次元スコアリング

すべてのツール呼び出しは3つの軸でスコアリングされます。重みは `src/ts/common.ts`（または `src/hooks/common.rs`）の `SCORE_WEIGHTS` で設定可能です：

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

すべての閾値は `src/ts/common.ts`（または `src/hooks/common.rs`）の設定可能な定数です：

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
    ↓ .harness/obs/session_{id}.jsonl
Analyze（SessionEnd）
    ↓ SessionAnalysis: ツール別、拡張子別、スコア分布
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed（4経路: パターン / 弱いツール / 弱いファイルタイプ / 高頻度エラー）
    ↓ .harness/evolved/{skill}/SKILL.md
Gate（フォーマットチェック、重複排除、上限10、停滞チェック）
    ↓ .harness/evolved_backup/（最良チェックポイント）
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

`.harness/guard-rules.yaml` でプロジェクト固有の安全ルールを追加できます：

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

ルールは組み込みガード（mainへのforce-push、rm -rf /、本番DBのDROP）とマージされます。

## クロスプロジェクト学習

プロジェクト間で失敗パターンを共有するオプトイン機能：

```bash
touch .harness/.cross-project-enabled  # オプトイン
```

有効化すると：
- セッション終了時に匿名化されたパターンを `~/.harness-global/patterns.jsonl` にエクスポート
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

## プロジェクトデータ（`.harness/`）

epic harnessはプロジェクトに `.harness/` ディレクトリを作成します：

```
.harness/
├── memory/           # プロジェクトパターンとルール（永続）
├── sessions/         # セッションスナップショット（復元用）
├── obs/              # ツール使用観測ログ（JSONL、セッション別）
├── evolved/          # 自動進化スキル
├── evolved_backup/   # 最良チェックポイント（停滞ロールバック用）
├── dispatch/         # スキルディスパッチログ（JSONL）
├── team/             # /team で生成されたエージェントとスキル
├── evolution.jsonl   # 完全な進化履歴
├── metrics.json      # 集約統計 + スキル帰属
└── guard-rules.yaml  # カスタムガードルール（オプション）
```

`.harness/` を `.gitignore` に追加するか、コミットするかはお任せします。

## 開発

### Rust（メイン — 約4倍高速）

```bash
cargo install --path .          # ビルド + ~/.cargo/bin/ にインストール
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # プラグインバイナリを更新
```

### Node.js（フォールバック）

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### フックのディスパッチ方法

`hooks.json` の各フックは3箇所でRustバイナリを探し、見つからない場合はNode.jsにフォールバックします：

```
1. プラグインローカル: hooks/bin/epic-harness
2. PATH:              ~/.cargo/bin/epic-harness（cargo install経由）
3. フォールバック:     node hooks/scripts/<hook>.js
```

### テスト

```bash
cargo test       # 98件のRustユニットテスト
npm test         # Node.js ユニット + e2eテスト
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
