<h1 align="center">Epic Harness</h1>

<blockquote><p align="center">自己進化するAIコーディングエージェントハーネス — 3個のコマンド、26個のスキル、1つの自律パイプライン、あなたの失敗から学習します。</p></blockquote>

<p align="center"><b>覚えるべき操作は少なく。キーストローク当たりの知性は高く。セッションを重ねるほどスマートに。</b></p>

<p align="center">
<a href="../../README.md">English</a> | <a href="../ja/README.md">日本語</a> | <a href="../ko/README.md">한국어</a> | <a href="../de/README.md">Deutsch</a> | <a href="../fr/README.md">Français</a> | <a href="../zh-CN/README.md">简体中文</a> | <a href="../zh-TW/README.md">繁體中文</a> | <a href="../pt-BR/README.md">Português</a> | <a href="../es/README.md">Español</a> | <a href="../hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="https://github.com/epicsagas/epic-harness/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/epicsagas/epic-harness?style=for-the-badge&labelColor=0d1117&color=ffd700&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/epic-harness/network/members"><img alt="Forks" src="https://img.shields.io/github/forks/epicsagas/epic-harness?style=for-the-badge&labelColor=0d1117&color=2ecc71&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/epic-harness/issues"><img alt="Issues" src="https://img.shields.io/github/issues/epicsagas/epic-harness?style=for-the-badge&labelColor=0d1117&color=ff6b6b&logo=github&logoColor=white" /></a>
  <a href="https://github.com/epicsagas/epic-harness/commits/main"><img alt="Last commit" src="https://img.shields.io/github/last-commit/epicsagas/epic-harness?style=for-the-badge&labelColor=0d1117&color=58a6ff&logo=git&logoColor=white" /></a>
</p>
<p align="center">
  <a href="../../LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-3fb950?style=for-the-badge&labelColor=0d1117" /></a>
  <img alt="Version" src="https://img.shields.io/badge/version-0.7.0-fc8d62?style=for-the-badge&labelColor=0d1117" />
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.82+-d73a49?style=for-the-badge&labelColor=0d1117&logo=rust&logoColor=white" />
  <img alt="Claude Code" src="https://img.shields.io/badge/Claude_Code-plugin-bc8cff?style=for-the-badge&labelColor=0d1117" />
  <a href="https://buymeacoffee.com/epicsaga"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/buy_me_a_coffee-FFDD00?style=for-the-badge&labelColor=0d1117&logo=buymeacoffee&logoColor=black" /></a>
</p>

Claude Codeプラグインで、**30以上のコマンドを3個のコマンド + 26個の自動トリガースキルに統合**し、自分の失敗パターンから**新しいスキルを進化**させます。

<p align="center">
  <img src="../../assets/features.png" alt="epic harness features" width="100%" />
</p>

---

![Demo](../../docs/demo/demo.gif)

### Webダッシュボード — セッション開始時に自動起動

evalスコア、ツール統計、orbitパイプライン、進化スキル、フックヘルスの10画面リアルタイムメトリクス。最初のClaude Codeセッションで自動的に開きます — 手動セットアップ不要。

<p align="center">
  <img src="../../assets/dashboard.png" alt="Dashboard" width="49%" />
  <img src="../../assets/dashboard-orbit.png" alt="Orbit Pipeline" width="49%" />
</p>

```bash
# 最初のセッションで自動起動（デフォルト: http://localhost:7700）
# ~/.harness/config.toml でポート設定または無効化:
[dashboard]
port = 7700       # 0 に設定すると自動起動を無効化
auto_open = true  # 最初のセッションでブラウザを開く
```

画面: **ダッシュボード** · /orbit パイプライン · コマンド（3） · スキル（26） · ライブエージェント · Eval & Evolve · フック（6） · インテグレーション（6） · harness-mem · 設定

---

## できること

1つのコマンドで機能をエンドツーエンドでシップできます。スキルはあなたが指示しなくても自動発火。エージェントは毎セッション確実にスマートになります。

```bash
$ /orbit "ログインAPIにJWT認証を追加"
→ spec approved → go (TDD subagents) → check (PASS) → ship (PR + CI) → evolve
```

パイプラインスキルを直接呼び出すこともできます:

```bash
/spec "ログインAPIにJWT認証を追加"   # 要件を明確化 → SPEC-*.md
/go                                   # 自動計画 → TDDサブエージェント → 4分
/check                                # 並列レビュー + セキュリティ + テスト → PASS
/ship                                 # 分離テスト → PR → CIグリーン
```

スキルはバックグラウンドで自動トリガー — 追加コマンド不要:

```
機能開発中?              → tdd 発火 (Red→Green→Refactor を強制)
テスト失敗?              → debug 発火 (まず根本原因、やみくも修正なし)
auth/DB を変更?          → secure 発火 (OWASPチェックリスト、近道なし)
ファイルが200行超?        → simplify 発火 (抽出・リネーム・簡素化)
```

セッション終了後、**evolveループ**が何が壊れたかを分析し、狙いを絞ったスキルを生成して次回セッションで読み込みます。今日 TypeScript ビルドで詰まったエージェントは、次回 `evo-ts-care` スキルを持っています。

---

## インストール

> **初めての方は** [クイックスタートガイド（5分）](../../docs/quickstart.md)をお読みください。

epic-harnessは**プラグイン**として配布されます — スキル、フック、`harness-mem` MCPサーバーはプラグインレイアウト（`skills/`, `hooks.json`, `mcp_config.json`）から直接ロードされます。`install` サブコマンドはなく、各ツールがディスクからプラグインを読み取ります。

### Claude Code（推奨）

```
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

バイナリ、スキル、フック、`harness-mem` MCPサーバーを一度に自動インストールします。

### agy（Antigravity CLI）

```bash
agy plugin install .
```

27個のスキル、フック、`harness-mem` MCPサーバーがプラグインの `plugin.json` + `skills/` + `hooks.json` + `mcp_config.json` から自動検出されます。

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

スキルとエージェントがすぐに利用可能 — 追加手順不要。

### バイナリのみ（プラグインホストなし）

```bash
brew install epicsagas/tap/epic-harness      # macOS / Linux (Homebrew)
cargo binstall epic-harness                  # プリビルドバイナリ (Rust)
cargo install epic-harness                   # ソースからビルド
```

Homebrewがない場合はインストーラースクリプトを使用:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/epic-harness/releases/latest/download/install.sh | sh
```

Windows:

```powershell
irm https://github.com/epicsagas/epic-harness/releases/latest/download/install.ps1 | iex
```

バイナリは初回フック実行時に `~/.harness/config.toml` と `HARNESS.md` を自動シードします — セットアップウィザードや `install` 手順は不要です。

> `epic-harness --version` で確認。`brew upgrade epic-harness` またはインストーラースクリプトの再実行で更新。

前提条件: **Git**。ソース/バイナリインストールには [Rustツールチェーン](https://rustup.rs) も必要です。

### 確認

```bash
epic --version              # バイナリがインストール済み
ls ~/.harness/              # データディレクトリ（初回セッションで自動作成）
```

Claude Codeセッション内: `/evolve status`

> **テレメトリ**: 使用状況レポートはデフォルトで有効（opt-out）です。`epic-harness telemetry status|on|off` で切り替え。

---

## テレメトリ

epic-harnessはフックの信頼性とスキル進化の改善のため、デフォルトで**匿名**の使用テレメトリを収集します（opt-out）。イベントは Posthog に送信されます。

**収集するもの:** コマンド名、実行時間、結果（成功/失敗）、失敗分類、フック遮断/失敗イベント — および `product`、`product_version`、`os`、ランダムな `install_id`（初回実行時に生成された UUID、`~/.config/epic-harness/install-id` に保存）。

**収集しないもの:** ソースコード、ファイル内容、ファイルパス、環境変数、シークレット、個人識別情報。

**制御:**

```bash
epic-harness telemetry status   # 現在の同意状態を表示
epic-harness telemetry off      # 無効化（送信を即時停止）
epic-harness telemetry on       # 再び有効化
```

同意は `~/.config/epic-harness/telemetry-consent` に保存されます。off の場合、テレメトリは送信されません。

---

## コマンド

| コマンド | 機能 |
|---------|------|
| `/orbit` | **完全自律パイプライン**: spec → go → check → ship → evolve を一括実行 |
| `/team` | orgのライブラリを閲覧、既存チームを雇用、または新規設計（3–6エージェント、`.claude/agents/` に同期） |
| `/evolve` | 手動進化トリガー — セッション分析、ダッシュボード表示、スキル有効性確認、ロールバック |

パイプラインステージ（`/spec`、`/go`、`/check`、`/ship`、`/discover`）は**スキル**になりました — コンテキストに応じて自動トリガーされるか、名前で直接呼び出せます。従来のコマンド名はエイリアスルーティングで引き続き動作します。

---

## /orbit — 自律パイプライン

`/orbit` はパイプライン全体を単一の自律実行にまとめます。モードを選ぶだけで、PRまでは完全にハンズフリーです。

```mermaid
flowchart TD
    START(["/orbit"]) --> MODE{"requirement?"}:::human
    MODE -->|"unclear"| WAIT["Interactive\n/discover → /spec\nthen 'orbit go'"]:::human
    MODE -->|"clear + complex"| COUNCIL["Council\n4-voice auto-spec"]:::auto
    MODE -->|"clear + simple"| DIRECT["Direct\nauto-spec"]:::auto
    WAIT --> SPEC_LOAD["Load spec"]
    COUNCIL --> SPEC_LOAD
    DIRECT --> SPEC_LOAD
    SPEC_LOAD --> GO["Go\nplan → TDD → integrate"]:::auto
    GO --> CHECK["Check\nreview + audit + test"]:::auto
    CHECK -->|"PASS / WARN"| SHIP["Ship\nisolated test → PR → CI"]:::auto
    CHECK -->|FAIL| RETRY{"retry < 3?"}
    RETRY -->|yes| GO
    RETRY -->|no| PAUSE["Pause\nuser decides"]:::human
    PAUSE -->|continue| GO
    PAUSE -->|abort| ABORT(["Abort"])
    SHIP --> EVOLVE["Evolve\nauto-analyze session"]:::auto
    EVOLVE --> DONE(["Orbit Complete\nconsolidated report"]):::auto

    classDef human fill:#4a4a6a,stroke:#9b9bcc,color:#fff
    classDef auto  fill:#1a5c3a,stroke:#4caf7d,color:#fff
```

**紫** — ヒューマンステップ: モード選択（unclear → インタラクティブ）、3回のチェック失敗時の一時停止。
**緑** — clear + complex → council自動spec; clear + simple → 直接ビルド; どちらも完全に自律。

状態は `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` に保持され、コンテキスト圧縮後も維持されます。

> **注意**: orbit自体の変更やドキュメントのみの編集時、エージェントがパイプラインをバイパスする場合があります。[既知の問題（エージェント判断）](#既知の問題エージェント判断)を参照してください。

---

## 自動スキル（Ring 2）

スキルはコンテキストに基づいて自動的にトリガーされます。手動で呼び出す必要はありません。

| スキル | トリガー条件 |
|-------|-------------|
| **spec** | 要件の定義が必要な時 — 番号付きR + ACドキュメントに変換 |
| **go** | ビルドフェーズ — 自動計画 → TDDサブエージェント → 並列実行 → AC検証 |
| **check** | レビューフェーズ — 並列コードレビュー + セキュリティ監査 + テスト、スコープ別追加項目 |
| **ship** | 出荷フェーズ — 分離テスト → フルチェックレポート付きPR → CI監視 + 自動修正 |
| **audit** | フル監査 — 並列コード品質 + セキュリティ + テストレビュー、セマンティック重複排除 |
| **eval** | ベースライン比較による品質回帰評価 — 正確性、パフォーマンス、品質 |
| **tdd** | 新機能の実装またはバグ修正 |
| **debug** | テスト失敗またはランタイムエラー |
| **discover** | 曖昧なリクエスト、問題のないソリューション、焦点の定まらない不満 |
| **secure** | auth / DB / API / シークレットのコードに触れた場合 |
| **threat-model** | セキュリティスコープ — 信頼境界の列挙、脅威アクター、シナリオ → THREAT_MODEL.md |
| **vuln-scan** | 体系的脆弱性スキャン — インジェクション、認証、データ露出、依存関係 → VULN-FINDINGS.json |
| **triage** | 敵対的検証 — 重要度調整、チェーン分析、根本原因グルーピング → TRIAGE.json |
| **perf** | ループ、クエリ、レンダリング、バッチ操作 |
| **simplify** | ファイルが200行超または高サイクロマティック複雑度 |
| **document** | パブリックAPIの追加またはシグネチャ変更 |
| **verify** | `/go` または `/ship` 完了前 |
| **context** | コンテキストウィンドウが70%超 |
| **council** | 曖昧なアーキテクチャまたは設計の決定 |
| **orchestrate** | マルチエージェントオーケストレーションステータスとライブエージェント介入 |
| **agent-introspection** | 3回以上の連続失敗または循環リトライパターン |
| **reflect** | オンデマンド: AIを思考増幅器として使っているか? 冷徹な証拠ベースの自己評価 |
| **commit** | Conventional Commits生成 — git diffから自動生成 |

> **トークン予算に関する注意:** Claude Codeはスキルの説明をすべてのセッションコンテキストに読み込みます。epicの26スキルはデフォルトの `skillListingBudgetFraction: 0.01`（1%）に収まります。追加スキル（episteme、alcove、obscuraなど）をインストールすると、合計が予算を超えて「descriptions dropped」警告が出る場合があります。`~/.claude/settings.json` に以下を追加して解決:
>
> ```json
> "skillListingBudgetFraction": 0.02
> ```
>
> 20以上のスキルをインストールしている場合は `0.03` を使用してください。

---

## 進化（Ring 3）

ハーネスはすべてのツール呼び出しを監視し、3軸でスコアリングし、失敗パターンを検出し、狙いを絞ったスキルを自動的に生成します — セッション終了時に。

### スコアリング

```
composite = 0.5 × tool_success + 0.3 × output_quality + 0.2 × execution_cost
```

失敗の分類（9種類）: `type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### パターン検出

| パターン | 検出内容 | デフォルト閾値 |
|---------|---------|---------------|
| `repeated_same_error` | 同じエラーがN回以上 | 3 |
| `fix_then_break` | 編集成功 → ビルド/テスト失敗 | ルックバック3、2サイクル |
| `long_debug_loop` | 同じファイルでスタック | 5操作 |
| `thrashing` | 編集↔エラーの交互発生 | 3回の編集、3回のエラー |

### 進化フロー

```
Observe (PostToolUse — 3-axis scoring)
    ↓ obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ per-tool, per-ext scores + patterns
Propose (Solver — graduated by score: ≥0.90 skip, ≥0.70 moderate, <0.70 full)
    ↓ SkillProposal[] with confidence
Curate (Accept/Merge/Skip, feedback masked from solver)
    ↓ evolved/{skill}/SKILL.md + meta.json
Gate (format check, dedup, cap 10, gated promotion ≥ 3 sessions)
    ↓ evolved_backup/ (best checkpoint)
Instinct (high-success patterns → cross-project memory.db nodes)
    ↓
Reload (next session — resume loads evolved skills)
```

スキルシーディング: 弱いツール（成功率 <60%、最低5観測）、弱いファイルタイプ（成功率 <50%、最低3観測）、高頻度エラー（5回以上）。

停滞: 5%改善なしで3セッション → ベストチェックポイントに自動ロールバック。

### SkillOptインスパイア最適化

[SkillOpt](https://arxiv.org/abs/2605.23904)から適応した3つのディープラーニングインスパイア手法:

| 手法 | 仕組み |
|------|--------|
| **ネガティブフィードバックバッファ** | 拒否された提案をTTLベースの有効期限付きで保存; 将来の提案は生成前にバッファと照合 |
| **ミニバッチリフレクション** | 観測を固定サイズのバッチに分解して構造的パターンを抽出; 優位エラー ≥60% + ≥2の異なるファイルで再利用可能 |
| **スロー/メタアップデート** | 直近5セッションの線形回帰でエポックを Improving / Regressing / PersistentFailure / StableSuccess に分類; パフォーマンス低下スキルを自動排除 |

### プロンプト自動チューニング

パフォーマンス低下の進化スキルは、`<!-- auto-tuned -->` 区切り文字の後にターゲット調整ガイダンスが追加されます。元のコンテンツは一切変更されません。3回連続でスコアが低下 → 自動ロールバックでチューニングを巻き戻し、履歴をクリア。

### スキル有効性

すべての進化スキルはA/Bアトリビューションで追跡されます:

```
/evolve history → Skill Effectiveness

| Skill              | With | Without | Delta |
|--------------------|------|---------|-------|
| evo-ts-care        | 0.87 | 0.72    | +15%  |
| evo-bash-discipline| 0.65 | 0.68    | -3%   |
```

正のデルタ = 有効。負 = `/evolve rollback` による削除を検討。

### コールドスタートプリセット

最初のセッションでは、スタックに適したプリセットスキルが自動適用されます:

| スタック | プリセット |
|--------|-----------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

### 本能学習

高成功率のパターンが抽出され、プロジェクト横断で促進されます:

```
observe (100% confirmed) → extract_instincts() → instinct node (confidence ≥ 0.8)
    → promote to global when observed in ≥ 2 projects
```

```bash
/evolve              # 今すぐ実行
/evolve status       # ダッシュボード: スコア、トレンド、パターン、スキル
/evolve history      # 完全な履歴 + スキル有効性
/evolve cross-project # クロスプロジェクトパターン分析
/evolve rollback     # 以前のベストを復元
/evolve reset        # すべての進化データをクリア
```

---

## セキュリティパイプライン

[defending-code](https://github.com/anthropics/defending-code-reference-harness)から移植した3段階の脆弱性評価パイプライン:

```bash
/threat-model    # 1. 信頼境界、脅威アクター、シナリオ → THREAT_MODEL.md
/vuln-scan       # 2. 4次元スキャナー（インジェクション、認証、データ露出、依存関係） → VULN-FINDINGS.json
/triage          # 3. 敵対的検証、重要度調整、チェーン分析 → TRIAGE.json
```

### 監査 `--strict` モード

セキュリティエンゲージメントでは、`--strict` モードが監査モード間の独立性を強制します:
- コード、セキュリティ、テストのレビューアーはdiff + specのみを受信 — ビルダーのコンテキストなし
- クロスチェックの独立性: 各モードは統合までブラインドで実行
- ブラインドスコアリングがアンカリングバイアスを防止

プロジェクトルートの `.harness/engagement.md` でオプションのエンゲージメントコンテキスト（認可、スコープ、制約、除外）を設定可能。テンプレートは `docs/references/engagement.md` を参照。

---

## フック（Ring 0）

見えない形で毎セッション実行されます。サブコマンド付きの単一Rustバイナリ（`epic-harness`）。

| フック | タイミング | 機能 |
|------|----------|------|
| **resume** | セッション開始 | コンテキストの復元、メモリの読み込み、スタックの検出 |
| **guard** | Bash実行前 | force-push-to-main、`rm -rf /`、DROP prod をブロック |
| **polish** | 編集後 | 自動フォーマット（Biome/Prettier/ruff/gofmt）+ 型チェック |
| **observe** | すべてのツール使用時 | `~/.harness/projects/{slug}/obs/` にログを記録（進化用） |
| **snapshot** | compact前 | `~/.harness/projects/{slug}/sessions/` に状態を保存 |
| **reflect** | セッション終了 | 失敗を分析、進化スキルをシード、ゲート、本能を抽出 |

Polishはobserveにフィードバックします: フォーマット失敗 → `lint_fail`、TypeScriptエラー → `build_fail`。polishからエラーが来る場合でも、Edit→Errorスラッシングが検出されます。

各セッションは独自の `session_{date}_{pid}_{random}.jsonl` を書き込みます — 複数の並行セッションが互いのデータを破損することはありません。

### フックプロファイル

`~/.harness/config.toml` または `EPIC_HOOK_PROFILE` 環境変数経由:

| プロファイル | アクティブなフック |
|------------|-----------------|
| `minimal` | guard, observe, resume |
| `standard`（デフォルト） | 上記 + polish, reflect, snapshot |
| `strict` | すべてのフック + 将来のstrict専用チェック |

### カスタムガードルール

プロジェクトルートの `.harness/guard-rules.yaml` でプロジェクト固有のルールを追加:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

---

## チーム（`epic team`）

チームは**orgレベル**であり、プロジェクトに縛られません。任意のプロジェクトで `/team` を実行すると、エージェント定義の共有プールが充実します — 黙って上書きすることはありません。

```bash
epic team                              # インタラクティブ: スキャン → デザイン → 書き込み → 同期
epic team sync backend                 # エージェントを .claude/agents/backend/ にディスパッチ
epic team link backend                 # ディスパッチ + チーム設定にプロジェクトを登録
epic team list                         # 現在のorgのすべてのチーム
epic team list --org netflix           # 指定のorgのチーム
epic team show backend --playbook      # 設定 + 完全なプレイブック
epic team delete backend               # 現在のプロジェクトからのみ削除
epic team delete backend --global      # orgストアから永久に削除
```

同期後、エージェントは次のセッションで利用可能になります: `@domain-expert`、`@reviewer`、`@tester` など。

| タイプ | キーワード | デフォルトエージェント |
|------|----------|---------------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

マルチorg: `epic team --org netflix` — orgごとに別のトポロジー。

マージ戦略: 変更されたエージェントはプロンプトを表示（デフォルト: 既存を保持、`.history/` にバックアップ）。プレイブックは常に追記。

---

## マルチツールサポート

すべてのツールが同じ `~/.harness/projects/{slug}/` データディレクトリを共有します。

| ツール | Ring 0 フック | コマンド | スキル | エージェント |
|------|-------------|----------|--------|--------|
| **Claude Code** | ✓ フル | ✓ 3コマンド（/orbitを含む） | ✓ 26スキル | Live |
| **Codex CLI** | ✓ フル¹ | ✓ 3プロンプト（/orbitを含む） | ✓ 26 | — |
| **Antigravity** | ✓ 部分² | ✓ 3コマンド（/orbitを含む） | ✓ 26 | — |

¹ `~/.codex/config.toml` で `plugin_hooks = true` · ² PreInvocation/PostInvocationのみ — PreToolUseなし（guard/polish利用不可）

---

## アーキテクチャ: 4-Ring モデル

```mermaid
flowchart TB
    subgraph R0["Ring 0 — Autopilot (hooks, invisible)"]
        direction LR
        h1(resume) --- h2(guard) --- h3(polish) --- h4(observe) --- h5(snapshot) --- h6(reflect)
    end

    subgraph R1["Ring 1 — Commands (you call these)"]
        direction TB
        subgraph orbit_wrap["  /orbit  "]
            direction LR
            c1("spec") --> c2("go") --> c3("check") --> c4("ship") --> c5("evolve")
        end
        c6("/team")
        c7("/evolve (manual)")
    end

    subgraph R2["Ring 2 — Auto Skills (context-triggered)"]
        direction LR
        s1(spec) --- s2(go) --- s3(check) --- s4(ship) --- s5(tdd) --- s6(debug) --- s7(secure) --- s8(perf) --- s9(simplify) --- s10(verify) --- s11(audit) --- s12(eval) --- s13(threat-model) --- s14(vuln-scan) --- s15(triage)
    end

    subgraph R3["Ring 3 — Evolve (self-improving)"]
        direction LR
        e1(observe) --> e2(analyze) --> e3(seed) --> e4(gate) --> e5(reload)
    end

    R0 -->|"observe every tool call"| R3
    R3 -.->|"evolved skills"| R2
    R1 -->|"auto-trigger skills"| R2
    R0 -->|"resume: restore context"| R1
```

---

## クロスプロジェクト学習

プロジェクト横断で失敗パターンを共有するにはオプトインします:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled
```

セッション終了 → 匿名化されたパターンを `~/.harness/global_patterns.jsonl` にエクスポート。セッション開始 → 他のプロジェクトの弱い領域からのヒントを表示。

---

## 統合メモリ

すべてのエージェントが `~/.harness/memory.db`（全文検索付きSQLite）のナレッジグラフを共有します。外部ランタイム不要。

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

### CLI

```bash
epic mem recall "auth refactor" --project my-project   # スマートリコール
epic mem add --title "JWT rotation" --type decision    # ノードを追加
epic mem search "JWT"                                  # FTS5検索
epic mem list --type decision --project my-project     # フィルター
epic mem context --project my-project                  # プロジェクトコンテキスト
epic mem serve                                         # Web UI → :7700 または --port 8800 でカスタムポート
epic mem mcp-install                                   # MCPサーバーを登録
epic mem export --out ./docs/memory                    # Markdownにエクスポート
```

### MCPツール（6）

| ツール | 目的 |
|------|------|
| `mem_recall` | ヒント + プロジェクト + グラフ隣接ノードによるスマートコンテキストリコール |
| `mem_add` | タイプ別自動重要度でノードを追加（または明示的な0.0–1.0） |
| `mem_search` | キーワード検索（全文）、重要度でランク付け |
| `mem_query` | タグ/タイプ/プロジェクトでフィルター |
| `mem_context` | プロジェクトスコープのスマートリコール（ヒントなし） |
| `mem_related` | ノードIDからのグラフトラバーサル（接続された知識を検索） |

### ノードタイプ

| タイプ | 作成者 | 重要度 |
|------|--------|--------|
| `decision` | 手動 / MCP | 0.9 |
| `resolution` | 手動 / MCP | 0.8 |
| `concept` | 手動 / MCP | 0.7 |
| `project` | 手動 / MCP | 0.7 |
| `instinct` | 自動（reflect） | 0.7 |
| `pattern` | 自動（reflect） | 0.5 |
| `error` | 自動（reflect） | 0.4 |
| `session` | 自動（reflect） | 0.2 |

ライフサイクル: アクセスなしで30日以上 → 重要度が10%低下（最低0.05）。180日以上 → `stale` タグが付き、リコールから除外。`pinned` タグは低下を防止。

---

<details>
<summary><strong>プロジェクトデータ — ディレクトリレイアウト</strong></summary>

## プロジェクトデータ

すべてのデータはプロジェクトルートではなく `~/.harness/`（ホームディレクトリ）に存在します。プロジェクト削除後も維持され、gitの履歴を汚染しません。

```
~/.harness/
├── memory.db                  # SQLiteナレッジグラフ（ノード + エッジ + FTS5）
├── graph.json                 # キャッシュされたグラフ（Web UI用）
├── config.toml                # ユーザー設定
├── global_patterns.jsonl      # クロスプロジェクトパターン（オプトイン）
├── orgs/                      # チームグローバルストア
│   └── {org}/teams/{team}/
│       ├── config.json, mission.md, playbook.md, agents/, .history/
└── projects/{slug}/
    ├── memory/                # プロジェクトパターンとルール
    ├── sessions/              # セッションスナップショット（resume用）
    ├── obs/                   # ツール使用観察ログ（JSONL）
    ├── evolved/               # 自動進化スキル
    │   ├── manifest.json
    │   └── {skill}/SKILL.md + meta.json
    ├── evolved_backup/        # ベストチェックポイント（ロールバック用）
    ├── dispatch/              # スキルディスパッチログ
    ├── evolution.jsonl        # 完全な進化履歴
    └── metrics.json           # 集計統計 + スキルアトリビューション
```

安全ルールをチームと共有: プロジェクトルートの `.harness/guard-rules.yaml`（gitにコミット）。

</details>

---

<details>
<summary><strong>設定 — config.toml リファレンス</strong></summary>

## 設定

`~/.harness/config.toml` のすべての調整可能なパラメーター。不在 = ハードコードされたデフォルト。

```toml
# 優先順位: 環境変数（EPIC_HOOK_PROFILE）> このファイル > デフォルト

[hook]
profile = "standard"         # "minimal" | "standard" | "strict"
gateguard_hints = true

[scoring]
weights = [0.5, 0.3, 0.2]   # [success, quality, cost]

[evolution]
max_skills = 10
stagnation_limit = 3
improvement_threshold = 0.05
gated_promotion_min = 3

[pattern]
# repeated_error_min = 3
# debug_loop_min = 5
# graduated_scope_skip = 0.90
# graduated_scope_moderate = 0.70

[instinct]
# confidence_threshold = 0.8
# promotion_min_projects = 2
# max_instincts = 20
# min_observations = 10
# min_avg_score = 0.5
```

</details>

---

## 既知の問題（エージェント判断）

コードのバグではなく、エージェントのコンテキスト解釈によって発生する問題です。ユーザーが注意すべき点をまとめています。

### 発見された問題

| 問題 | 発生条件 | 現象 | 回避策 |
|------|---------|------|--------|
| **Orbit自己変更のバイパス** | `/orbit`にorbit自体の改善を要求 | エージェントがorbitパイプライン全体をスキップし、mainにアドホックで編集。spec/PR/トレーサビリティなしで変更が未コミット状態に | orbit完了後に`git status`を確認。パイプライン状態なしでmainに変更がある場合、手動コミットするか別ブランチで`/orbit`を再実行 |
| **ドキュメントのみのタスクでプロトコル省略** | `/orbit`にMarkdownのみの変更（テスト対象コードなし）を指示 | エージェントがTDD/テストフェーズを無意味と判断し、パイプライン全体をスキップ | 純粋なドキュメント変更は許容可能。コード+ドキュメントの混在時は、コード関連フェーズがスキップされていないか確認 |
| **モード誤分類** | DirectとCouncilの境界にあるリクエスト | Council（4ボイス）が適切な場面でDirectを選択、またはその逆 | エージェントの選択が不適切に思える場合は、「Councilモードを使用」または「Directモードを使用」と明示的に指定 |

### 意図的な設計選択

強化を検討したが、評価後に現状維持とした項目:

| 選択 | 強化しなかった理由 | 根拠 |
|------|-------------------|------|
| **Goフェーズでのみワークツリーに入る** | preflightから隔離可能 | Preflight/mode/specは読み取り専用。早期隔離はメリットなく複雑さが増加 — ブランチ作成自体がGoフェーズ |
| **Ship後にワークツリーを保持** | PRマージ後に自動削除可能 | ブランチがPRヘッド。マージ前の削除はPRを壊す。クリーンアップはユーザーがマージ後に実施 |
| **ブランチ名が`feature/{slug}`ではなく`orbit-{slug}`** | 命名規則に合わせ可能 | `EnterWorktree`が名前に`/`を許可しない。作成後のリネームは見た目のメリットのみで手順が増える |
| **ドキュメントのみの変更に軽量パイプラインなし** | doc-only検出でTDD/テストスキップ可能 | 検出が不安定（「ドキュメント」の定義が曖昧）。限界的な利益に対してプロトコルの複雑さが増加 |

---

## トラブルシューティング

<details>
<summary>インストール後に command not found: epic となる</summary>

CargoのbinディレクトリをPATHに追加:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

この行を `~/.zshrc` または `~/.bashrc` に追加して永続化してください。
</details>

<details>
<summary>Claude Codeでフックが実行されない</summary>

プラグインを再インストールしてフックを再読み込み:

```
/plugin install epic@epicsagas
```

その後Claude Codeを再起動。フックはプラグインの `hooks.json` から読み込まれます。
</details>

<details>
<summary>macOSで Permission denied（Gatekeeper）</summary>

macOSがインターネットからダウンロードされた未署名バイナリをブロックする場合があります:

```bash
xattr -d com.apple.quarantine ~/.cargo/bin/epic-harness
xattr -d com.apple.quarantine ~/.cargo/bin/epic
```
</details>

<details>
<summary>epic: プラグインフック内でバイナリが見つからない</summary>

プラグインはまず `hooks/bin/epic-harness` でバイナリを探します。`cargo install` で更新した後、コピーしてください:

```bash
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness
```
</details>

---

## 開発

```bash
cargo install --path .                                        # ビルド + インストール
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness           # プラグインバイナリを更新
cargo test                                                    # テスト
```

フックはバイナリを2か所で探します: `hooks/bin/epic-harness`（プラグインローカル）→ `~/.cargo/bin/epic-harness`（PATH）。

---

## リンク

- [変更履歴](../../CHANGELOG.md) — リリース履歴
- [コントリビューション](../../CONTRIBUTING.md) — コントリビューション方法
- [セキュリティ](../../SECURITY.md) — 脆弱性の報告
- [Issues](https://github.com/epicsagas/epic-harness/issues) — バグレポートと機能リクエスト

## 謝辞

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — 自動進化とベンチマークパターン
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Codeエージェントスキルシステム
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — 包括的なClaude Codeパターン
- [gstack](https://github.com/garrytan/gstack) — プラグインアーキテクチャの参考
- [harness](https://github.com/revfactory/harness) — フックとハーネスのインフラパターン
- [serena](https://github.com/oraios/serena) — 自律エージェント設計
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — マルチコマンドフレームワークアーキテクチャ
- [superpowers](https://github.com/obra/superpowers) — Claude Code拡張パターン

## ライセンス

[Apache 2.0](../../LICENSE)
