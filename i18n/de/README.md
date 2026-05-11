<h1 align="center">Epic Harness</h1>

<blockquote><p align="center">Ein selbstentwickelndes KI-Coding-Agent-Harness — 8 Befehle, 1 autonome Pipeline, automatisch ausgelöste Skills, lernt aus Ihren Fehlern.</p></blockquote>

<p align="center"><b>Weniger zu merken. Mehr Intelligenz pro Tastendruck. Wird mit jeder Session intelligenter.</b></p>

<p align="center">
<a href="../../README.md">English</a> | <a href="../ja/README.md">日本語</a> | <a href="../ko/README.md">한국어</a> | <a href="README.md">Deutsch</a> | <a href="../fr/README.md">Français</a> | <a href="../zh-CN/README.md">简体中文</a> | <a href="../zh-TW/README.md">繁體中文</a> | <a href="../pt-BR/README.md">Português</a> | <a href="../es/README.md">Español</a> | <a href="../hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/License-Apache_2.0-blue.svg" alt="Lizenz"></a>
  <img src="https://img.shields.io/badge/Version-0.3.8-brightgreen.svg" alt="Version">
  <img src="https://img.shields.io/badge/Rust-1.82+-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Claude_Code-Plugin-purple.svg" alt="Claude Code Plugin">
  <a href="https://buymeacoffee.com/epicsaga"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

Ein Claude Code Plugin, das **30+ Befehle durch 8 ersetzt**, **Skills automatisch auslöst** basierend auf dem, was Sie gerade tun, und **neue Skills entwickelt** aus Ihren eigenen Fehlermustern.

<p align="center">
  <img src="../../assets/features.png" alt="Epic Harness Funktionen" width="100%" />
</p>

---

![Demo](../../docs/demo/demo.gif)

---

## Was es macht

Ein einziger Befehl liefert ein Feature von Ende zu Ende. Skills werden ausgelöst, ohne dass Sie danach fragen. Der Agent wird nach jeder Session intelligenter.

```bash
$ /orbit "JWT-Auth zur Login-API hinzufügen"
→ spec genehmigt → go (TDD-Subagents) → check (PASS) → ship (PR + CI) → evolve
```

Oder Schritt für Schritt manuell:

```bash
/spec "JWT-Auth zur Login-API hinzufügen"   # klärt Anforderungen → SPEC-*.md
/go                                          # plant automatisch → TDD-Subagents → 4 Min.
/check                                       # paralleles Review + Sicherheit + Tests → PASS
/ship                                        # isolierter Test → PR → CI grün
```

Skills werden automatisch im Hintergrund ausgelöst — keine zusätzlichen Befehle nötig:

```
Feature schreiben?        → tdd wird ausgelöst (Red→Green→Refactor erzwungen)
Test schlägt fehl?        → debug wird ausgelöst (Ursachenanalyse zuerst, keine zufälligen Fixes)
Auth oder DB berührt?     → secure wird ausgelöst (OWASP-Checkliste, keine Abkürzungen)
Datei erreicht 200 Zeilen? → simplify wird ausgelöst (extrahieren, umbenennen, reduzieren)
```

Nachdem die Session endet, analysiert die **evolve-Schleife**, was fehlschlug, generiert zielgerichtete Skills und lädt sie in der nächsten Session. Der Agent, der mit TypeScript-Build-Fehlern zu kämpfen hatte, wird beim nächsten Mal einen `evo-ts-care`-Skill haben.

---

## Installation

> **Zum ersten Mal hier?** Lesen Sie den [Schnellstart-Leitfaden (5 Min.)](../../docs/quickstart.md).

### Claude Code (empfohlen)

```
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

Installiert das Binary automatisch und registriert alle Hooks in einem Schritt.

### macOS / Linux

```bash
brew install epicsagas/tap/epic-harness
```

Kein Homebrew? Verwenden Sie das Installationsskript:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.ps1 | iex
```

### Über die Rust-Toolchain

```bash
cargo binstall epic-harness   # vorgefertigtes Binary (schnell)
cargo install epic-harness    # aus dem Quellcode kompilieren
```

Führen Sie danach den Setup-Assistenten aus:

```bash
epic install          # Claude Code (Standard)
epic install codex    # Codex CLI
epic install gemini   # Gemini CLI
```

> `epic-harness --version` zur Überprüfung. Aktualisieren mit `brew upgrade epic-harness` oder durch erneutes Ausführen des Installationsskripts.

Voraussetzungen: **Git**. Quellcode-/Binary-Installationen benötigen zusätzlich die [Rust-Toolchain](https://rustup.rs).

### `epic install` — Setup-Assistent

Nach der Installation des Binaries führen Sie `epic install` (oder `epic install claude`) aus, um:

1. Die Verzeichnisstruktur `~/.harness/` zu erstellen
2. Befehle, Skills und Agents in das Konfigurationsverzeichnis des Tools zu synchronisieren
3. Den MCP-Server (harness-mem) für Claude Code zu registrieren
4. `~/.harness/config.toml` mit Standardeinstellungen zu erstellen, falls nicht vorhanden

Bei Claude Code wird `hooks/setup.sh` automatisch beim Session-Start ausgeführt und installiert das Binary, falls es fehlt. Nach dem anfänglichen Klonen ist kein manueller Schritt erforderlich.

### Andere Tools

```bash
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (erfordert Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/
epic install              # Interaktives Menü
```

Integrationsdateien werden **synchronisiert** vom Binary: fehlende oder veraltete Dateien werden geschrieben. `GEMINI.md` und `AGENTS.md` werden nur erstellt, wenn sie nicht vorhanden sind.

### Überprüfung

```bash
epic --version              # Binary installiert
ls ~/.harness/              # Datenverzeichnis existiert
```

In einer Claude Code-Session: `/evolve status`

---

## Befehle

| Befehl | Was er macht |
|---------|-------------|
| `/orbit` | **Vollständig autonome Pipeline**: spec → go → check → ship → evolve in einem Durchlauf |
| `/discover` | Zuerst das Problem Rahmen — 5-Whys, JTBD, Sokratisches Fragen (max. 3 Runden) |
| `/spec` | Anforderungen in ein nummeriertes R + AC-Dokument umwandeln, gespeichert als `SPEC-{timestamp}.md` |
| `/go` | Automatische Planung → TDD-Subagents → parallele Ausführung mit Worktree-Isolation → AC-Verifikation |
| `/check` | Paralleles Review + Sicherheitsaudit + Tests, mit bereichsbasierten Extras (API-Vertrag, Barrierefreiheit, Migrationssicherheit) |
| `/ship` | Isolierter Preflight-Test in einem sauberen Worktree → PR mit vollem Prüfbericht → CI-Überwachung + Auto-Fix |
| `/team` | Organisations-Bibliotheken durchsuchen, bestehende Teams einbinden oder neue entwerfen (3–6 Agents, synchronisiert zu `.claude/agents/`) |
| `/evolve` | Manueller Evolutions-Trigger — Sessions analysieren, Dashboard anzeigen, Skill-Effektivität prüfen, Rollback durchführen |

---

## /orbit — Autonome Pipeline

`/orbit` fasst die gesamte Pipeline in einer einzigen autonomen Ausführung zusammen. Wählen Sie einen Modus — alles andere läuft automatisch bis zum PR.

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

**Lila** — manuelle Schritte: Modusauswahl (unklar → interaktiv), 3× Prüf-Fehler Pause.
**Grün** — klar + komplex → Rat (Council) automatischer Spec; klar + einfach → direkter Build; beide vollständig autonom.

Der Status wird in `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` gespeichert — übersteht Context-Compaction.

> **Einschränkungen**: Der Agent kann die Pipeline umgehen, wenn er orbit selbst modifiziert oder nur Dokumente bearbeitet. Siehe [Bekannte Probleme (Agent-Beurteilung)](#bekannte-probleme-agent-beurteilung).

---

## Auto-Skills (Ring 2)

Skills werden automatisch basierend auf dem Kontext ausgelöst. Sie rufen sie nicht aktiv auf.

| Skill | Wird ausgelöst wenn |
|-------|---------------------|
| **tdd** | Neues Feature-Implementation oder Bug-Fix |
| **debug** | Testfehler oder Laufzeitfehler |
| **discover** | Vage Anfrage, Lösung ohne Problem, unfokussierte Beschwerde |
| **secure** | Auth / DB / API / Secrets-Code wird berührt |
| **perf** | Schleifen, Abfragen, Rendering, Batch-Operationen |
| **simplify** | Datei > 200 Zeilen oder hohe zyklomatische Komplexität |
| **document** | Öffentliche API hinzugefügt oder Signatur geändert |
| **verify** | Vor Abschluss von `/go` oder `/ship` |
| **context** | Context-Fenster > 70% |
| **council** | Mehrdeutige Architektur- oder Designentscheidungen |
| **agent-introspection** | 3+ aufeinanderfolgende Fehler oder kreisförmiges Wiederholungsmuster |
| **reflect** | Auf Abruf: Nutzen Sie KI als Gedankenverstärker? Kalte, evidenzbasierte Selbsteinschätzung |

---

## Evolve (Ring 3)

Das Harness überwacht jeden Tool-Aufruf, bewertet ihn auf 3 Achsen, erkennt Fehlermuster und generiert zielgerichtete Skills — automatisch, am Ende der Session.

### Bewertung

```
composite = 0.5 × tool_success + 0.3 × output_quality + 0.2 × execution_cost
```

Fehlerklassifizierung (9 Typen): `type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Mustererkennung

| Muster | Erkennt | Standard-Schwellenwert |
|---------|---------|------------------------|
| `repeated_same_error` | Gleicher Fehler N+ Mal | 3 |
| `fix_then_break` | Edit-Erfolg → Build/Test schlägt fehl | 3 Lookback, 2 Zyklen |
| `long_debug_loop` | Bei gleicher Datei festgefahren | 5 Operationen |
| `thrashing` | Edit↔Error im Wechsel | 3 Edits, 3 Errors |

### Evolutions-Ablauf

```
Observe (PostToolUse — 3-Achsen-Bewertung)
    ↓ obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ pro-Tool, pro-Erweiterung Bewertungen + Muster
Propose (Solver — gestaffelt nach Bewertung: ≥0.90 überspringen, ≥0.70 moderat, <0.70 vollständig)
    ↓ SkillProposal[] mit Konfidenz
Curate (Akzeptieren/Zusammenführen/Überspringen, Feedback vom Solver maskiert)
    ↓ evolved/{skill}/SKILL.md + meta.json
Gate (Formatprüfung, Dedup, Limit 10, gesteuerte Beförderung ≥ 3 Sessions)
    ↓ evolved_backup/ (bester Checkpoint)
Instinct (Erfolgreiche Muster → projektübergreifende memory.db-Knoten)
    ↓
Reload (nächste Session — Resume lädt entwickelte Skills)
```

Skill-Seeding: schwaches Tool (Erfolgsrate <60%, mind. 5 Beobachtungen), schwacher Dateityp (Erfolgsrate <50%, mind. 3 Beobachtungen), hochfrequenter Fehler (5+ Vorkommen).

Stagnation: 3 Sessions ohne 5% Verbesserung → automatischer Rollback zum besten Checkpoint.

### Skill-Effektivität

Jeder entwickelte Skill wird mit A/B-Attribution verfolgt:

```
/evolve history → Skill-Effektivität

| Skill              | Mit  | Ohne  | Delta |
|--------------------|------|-------|-------|
| evo-ts-care        | 0.87 | 0.72  | +15%  |
| evo-bash-discipline| 0.65 | 0.68  | -3%   |
```

Positives Delta = effektiv. Negatives = Entfernung über `/evolve rollback` in Betracht ziehen.

### Kaltstart-Vorlagen

Bei der ersten Session werden stack-gerechte Vorlagen-Skills automatisch angewendet:

| Stack | Vorlagen |
|-------|----------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

### Instinct-Lernen

Erfolgreiche Muster werden extrahiert und projektübergreifend gefördert:

```
observe (100% bestätigt) → extract_instincts() → Instinct-Knoten (Konfidenz ≥ 0.8)
    → global fördern wenn in ≥ 2 Projekten beobachtet
```

```bash
/evolve              # Jetzt ausführen
/evolve status       # Dashboard: Bewertungen, Trends, Muster, Skills
/evolve history      # Vollständiger Verlauf + Skill-Effektivität
/evolve cross-project # Projektübergreifende Musteranalyse
/evolve rollback     # Vorherigen besten Stand wiederherstellen
/evolve reset        # Alle Evolutionsdaten löschen
```

---

## Hooks (Ring 0)

Laufen unsichtbar bei jeder Session. Ein einzelnes Rust-Binary (`epic-harness`) mit Unterbefehlen.

| Hook | Wann | Was er macht |
|------|------|--------------|
| **resume** | Session-Start | Context wiederherstellen, Speicher laden, Stack erkennen |
| **guard** | Vor Bash | Force-Push-to-Main blockieren, `rm -rf /`, DROP prod |
| **polish** | Nach Edit | Auto-Formatierung (Biome/Prettier/ruff/gofmt) + Typprüfung |
| **observe** | Jede Tool-Nutzung | In `~/.harness/projects/{slug}/obs/` für Evolution protokollieren |
| **snapshot** | Vor Compact | Zustand in `~/.harness/projects/{slug}/sessions/` speichern |
| **reflect** | Session-Ende | Fehler analysieren, entwickelte Skills seeden, prüfen, Instincts extrahieren |

Polish meldet Ergebnisse zurück an observe: Formatierungsfehler → `lint_fail`, TypeScript-Fehler → `build_fail`. Edit→Error-Thrashing wird sogar erkannt, wenn die Fehler aus polish stammen.

Jede Session schreibt ihre eigene `session_{date}_{pid}_{random}.jsonl` — mehrere gleichzeitige Sessions beschädigen nicht gegenseitig ihre Daten.

### Hook-Profile

Über `~/.harness/config.toml` oder Umgebungsvariable `EPIC_HOOK_PROFILE`:

| Profil | Aktive Hooks |
|---------|-------------|
| `minimal` | guard, observe, resume |
| `standard` (Standard) | oben + polish, reflect, snapshot |
| `strict` | alle Hooks + zukünftige Strict-only-Prüfungen |

### Benutzerdefinierte Guard-Regeln

Projektspezifische Regeln über `.harness/guard-rules.yaml` im Projektverzeichnis hinzufügen:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace-Löschung blockiert
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — zuerst überprüfen
```

---

## Team (`epic team`)

Teams sind **Organisationsebene**, nicht projektgebunden. Das Ausführen von `/team` in einem beliebigen Projekt bereichert einen gemeinsamen Pool von Agent-Definitionen — überschreibt niemals stillschweigend.

```bash
epic team                              # Interaktiv: scannen → entwerfen → schreiben → synchronisieren
epic team sync backend                 # Agents bereitstellen → .claude/agents/backend/
epic team link backend                 # Bereitstellen + Projekt in Team-Konfiguration registrieren
epic team list                         # Alle Teams in der aktuellen Organisation
epic team list --org netflix           # Teams in einer benannten Organisation
epic team show backend --playbook      # Konfiguration + vollständiges Playbook
epic team delete backend               # Nur aus aktuellem Projekt entfernen
epic team delete backend --global      # Dauerhaft aus dem Organisations-Speicher löschen
```

Nach der Synchronisierung sind Agents in der nächsten Session verfügbar: `@domain-expert`, `@reviewer`, `@tester` usw.

| Typ | Schlüsselwort | Standard-Agents |
|------|---------------|-----------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

Multi-Org: `epic team --org netflix` — separate Topologie pro Organisation.

Merge-Strategie: geänderte Agents fragen nach (Standard: bestehende beibehalten, Backup in `.history/`). Playbook wird immer angehängt.

---

## Multi-Tool-Unterstützung

Alle Tools teilen dasselbe `~/.harness/projects/{slug}/`-Datenverzeichnis.

| Tool | Ring 0 Hooks | Befehle | Skills | Agents |
|------|-------------|----------|--------|--------|
| **Claude Code** | ✓ Vollständig | ✓ 8 Befehle (inkl. /orbit) | ✓ 11 Skills | ✓ 4 |
| **Codex CLI** | ✓ Vollständig¹ | ✓ 8 Prompts (inkl. /orbit) | ✓ 7 | ✓ 4 |
| **Gemini CLI** | ✓ Teilweise² | ✓ 8 Befehle (inkl. /orbit) | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Vollständig³ | ✓ 8 Befehle (inkl. /orbit) | ✓ über Rules | ✓ 4 |
| **OpenCode** | ✓ Teilweise⁴ | ✓ 8 Befehle (inkl. /orbit) | — | ✓ 4 |
| **Cline** | ✓ Vollständig⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ `codex_hooks = true` in `~/.codex/config.toml` · ² Guard auf `BeforeModel`-Ebene · ³ Cursor 1.7+ · ⁴ JS-Plugin · ⁵ 5 Hook-Skripte · ⁶ Nur Conventions

---

## Architektur: 4-Ring-Modell

```mermaid
flowchart TB
    subgraph R0["Ring 0 — Autopilot (Hooks, unsichtbar)"]
        direction LR
        h1(resume) --- h2(guard) --- h3(polish) --- h4(observe) --- h5(snapshot) --- h6(reflect)
    end

    subgraph R1["Ring 1 — Befehle (diese rufen Sie auf)"]
        direction TB
        subgraph orbit_wrap["  /orbit  "]
            direction LR
            c1("/discover") --> c2("/spec") --> c3("/go") --> c4("/check") --> c5("/ship") --> c6("/evolve")
        end
        c7("/team")
        c8("/evolve (manuell)")
    end

    subgraph R2["Ring 2 — Auto-Skills (kontextgesteuert)"]
        direction LR
        s1(tdd) --- s2(debug) --- s3(secure) --- s4(perf) --- s5(simplify) --- s6(verify) --- s7(council)
    end

    subgraph R3["Ring 3 — Evolve (selbstverbessernd)"]
        direction LR
        e1(observe) --> e2(analyze) --> e3(seed) --> e4(gate) --> e5(reload)
    end

    R0 -->|"observe every tool call"| R3
    R3 -.->|"evolved skills"| R2
    R1 -->|"auto-trigger skills"| R2
    R0 -->|"resume: restore context"| R1
```

---

## Projektübergreifendes Lernen

Optional aktivierbar, um Fehlermuster über Projekte hinweg zu teilen:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled
```

Session-Ende → exportiert anonymisierte Muster nach `~/.harness/global_patterns.jsonl`. Session-Start → zeigt Hinweise aus den Schwachstellen anderer Projekte.

---

## Vereinheitlichter Speicher

Alle Agents teilen einen Wissensgraphen in `~/.harness/memory.db` (SQLite mit Volltextsuche). Keine externe Laufzeitumgebung.

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

### CLI

```bash
epic mem recall "auth refactor" --project my-project   # Intelligenter Abruf
epic mem add --title "JWT rotation" --type decision    # Knoten hinzufügen
epic mem search "JWT"                                  # FTS5-Suche
epic mem list --type decision --project my-project    # Filtern
epic mem context --project my-project                  # Projekt-Kontext
epic mem serve                                         # Web UI → :7700 oder benutzerdefinierter Port mit --port 8800
epic mem mcp-install                                   # MCP-Server registrieren
epic mem export --out ./docs/memory                    # Nach Markdown exportieren
```

### MCP-Tools (6)

| Tool | Zweck |
|------|-------|
| `mem_recall` | Intelligenter kontextueller Abruf mit Hint + Projekt + Graph-Nachbarn |
| `mem_add` | Knoten hinzufügen mit automatischer Wichtigkeit nach Typ (oder explizit 0.0–1.0) |
| `mem_search` | Schlagwortsuche (Volltext), nach Wichtigkeit sortiert |
| `mem_query` | Filtern nach Tag/Typ/Projekt |
| `mem_context` | Projektbezogener intelligenter Abruf (ohne Hint) |
| `mem_related` | Graph-Traversierung ab einer Knoten-ID (findet verbundenenes Wissen) |

### Knoten-Typen

| Typ | Erstellt von | Wichtigkeit |
|------|-------------|------------|
| `decision` | Manuell / MCP | 0.9 |
| `resolution` | Manuell / MCP | 0.8 |
| `concept` | Manuell / MCP | 0.7 |
| `project` | Manuell / MCP | 0.7 |
| `instinct` | Auto (reflect) | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

Lebenszyklus: 30+ Tage ohne Zugriff → 10% Wichtigkeitsverfall (Minimum 0.05). 180+ Tage → als `stale` markiert, vom Abruf ausgeschlossen. `pinned`-Tag verhindert Verfall.

> **Web UI**: Die Graph-Visualisierung wird aktiv verbessert — Clustering, Nachbar-Hervorhebung und Offline-Fallback sind kürzlich hinzugekommen. Weitere Verbesserungen sind in Arbeit.

---

<details>
<summary><strong>Projektdaten — Verzeichnisstruktur</strong></summary>

## Projektdaten

Alle Daten befinden sich in `~/.harness/` (Home-Verzeichnis), nicht im Projektverzeichnis. Übersteht Projektlöschung, verschmutzt nicht die Git-Historie.

```
~/.harness/
├── memory.db                  # SQLite-Wissensgraph (Knoten + Kanten + FTS5)
├── graph.json                 # Zwischengespeicherter Graph (für Web UI)
├── config.toml                # Benutzerkonfiguration
├── global_patterns.jsonl      # Projektübergreifende Muster (optional)
├── orgs/                      # Globaler Team-Speicher
│   └── {org}/teams/{team}/
│       ├── config.json, mission.md, playbook.md, agents/, .history/
└── projects/{slug}/
    ├── memory/                # Projektmuster und Regeln
    ├── sessions/              # Session-Snapshots (für Resume)
    ├── obs/                   # Tool-Nutzungs-Beobachtungsprotokolle (JSONL)
    ├── evolved/               # Automatisch entwickelte Skills
    │   ├── manifest.json
    │   └── {skill}/SKILL.md + meta.json
    ├── evolved_backup/        # Bester Checkpoint (für Rollback)
    ├── dispatch/              # Skill-Dispatch-Protokolle
    ├── evolution.jsonl        # Vollständige Evolutionshistorie
    └── metrics.json           # Aggregierte Statistiken + Skill-Attribution
```

Sicherheitsregeln mit Ihrem Team teilen: `.harness/guard-rules.yaml` im Projektverzeichnis (in Git eingecheckt).

</details>

---

<details>
<summary><strong>Konfiguration — config.toml-Referenz</strong></summary>

## Konfiguration

Alle einstellbaren Parameter in `~/.harness/config.toml`. Fehlt = fest codierte Standards.

```toml
# Priorität: Umgebungsvariable (EPIC_HOOK_PROFILE) > diese Datei > Standards

[hook]
profile = "standard"         # "minimal" | "standard" | "strict"
gateguard_hints = true

[scoring]
weights = [0.5, 0.3, 0.2]   # [Erfolg, Qualität, Kosten]

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

## Bekannte Probleme (Agent-Beurteilung)

Diese Probleme entstehen durch die Interpretation des Kontexts durch den Agenten und nicht durch Fehler im Code. Hier aufgeführt, damit Benutzer wissen, worauf sie achten sollten.

### Entdeckte Probleme

| Problem | Wann | Was passiert | Workaround |
|---------|------|-------------|------------|
| **Orbit-Selbstmodifikation-Umgehung** | `/orbit` wird aufgefordert, orbit selbst zu verbessern | Der Agent kann die orbit-Pipeline vollständig überspringen und Dateien ad-hoc auf main bearbeiten, Änderungen uncommittet ohne Spec/PR/Nachverfolgbarkeit lassen | Nach Abschluss von orbit `git status` prüfen. Wenn Änderungen auf main ohne Pipeline-Status vorliegen, manuell committen oder `/orbit` von einem separaten Branch erneut ausführen |
| **Doc-only-Aufgabe überspringt Protokoll** | `/orbit` erhält eine reine Markdown-Änderung (kein Code zum Testen) | Der Agent kann TDD/Test-Phasen als bedeutungslos einstufen und die vollständige Pipeline überspringen | Akzeptabel für reine Dokumentänderungen. Bei gemischtem Code+Doc sicherstellen, dass der Agent keine codebezogenen Phasen überspringt |
| **Modus-Fehlklassifikation** | Anfrage ist grenzwertig zwischen Direct und Council | Der Agent kann Direct wählen, wenn Council (4-Stimmen) mehr Randfälle erkennen würde, oder Council, wenn Direct ausreicht | Wenn der Agent einen Modus wählt, der falsch erscheint, sagen Sie explizit "verwende Council-Modus" oder "verwende Direct-Modus" |

### Bewusste Designentscheidungen

Diese wurden als Erweiterung in Betracht gezogen, aber nach Evaluierung beibehalten:

| Entscheidung | Warum nicht erweitert | Begründung |
|-------------|----------------------|------------|
| **Worktree-Eintritt in der Go-Phase, nicht zu Orbit-Start** | Könnte früher isolieren | Preflight/Modus/Spec sind schreibgeschützt. Frühere Isolation erhöht die Komplexität ohne Nutzen — der Branch wird ohnehin erst in der Go-Phase erstellt |
| **Worktree nach Ship beibehalten** | Könnte nach PR-Merge automatisch entfernt werden | Der Branch ist der PR-Head. Entfernung vor Merge beschädigt den PR. Bereinigung wird dem Benutzer nach dem Merge überlassen |
| **Branch benannt `orbit-{slug}` nicht `feature/{slug}`** | Könnte konventioneller Branch-Namenskonvention entsprechen | `EnterWorktree` erlaubt kein `/` in Namen. Umbenennung nach Erstellung fügt einen Schritt für rein kosmetischen Nutzen hinzu |
| **Kein leichtgewichtiger Pipeline-Pfad für Doc-Änderungen** | Könnte Doc-only erkennen und TDD/Tests überspringen | Erkennung ist fragil (was zählt als "Doc"?). Ein separater Pfad erhöht die Protokollkomplexität für marginalen Gewinn |

---

## Fehlerbehebung

<details>
<summary>command not found: epic nach Installation</summary>

Fügen Sie das Cargo-Bin-Verzeichnis zu Ihrem PATH hinzu:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Fügen Sie diese Zeile zu Ihrer `~/.zshrc` oder `~/.bashrc` hinzu, um sie dauerhaft zu machen.
</details>

<details>
<summary>Hooks werden in Claude Code nicht ausgelöst</summary>

Führen Sie die Installation erneut aus, um Hooks in die Claude Code-Einstellungen zu synchronisieren:

```bash
epic install claude
```

Starten Sie dann Claude Code neu. Hooks werden in `~/.claude/settings.json` geschrieben.
</details>

<details>
<summary>Permission denied auf macOS (Gatekeeper)</summary>

macOS kann unsignierte, aus dem Internet heruntergeladene Binaries blockieren:

```bash
xattr -d com.apple.quarantine ~/.cargo/bin/epic-harness
xattr -d com.apple.quarantine ~/.cargo/bin/epic
```
</details>

<details>
<summary>epic: Binary in Plugin-Hooks nicht gefunden</summary>

Das Plugin sucht zuerst nach dem Binary in `hooks/bin/epic-harness`. Nach Aktualisierung über `cargo install` kopieren Sie es:

```bash
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness
```
</details>

---

## Entwicklung

```bash
cargo install --path .                                        # Kompilieren + installieren
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness           # Plugin-Binary aktualisieren
cargo test                                                    # Tests
```

Hooks suchen das Binary an zwei Orten: `hooks/bin/epic-harness` (Plugin-lokal) → `~/.cargo/bin/epic-harness` (PATH).

---

## Links

- [Changelog](../../CHANGELOG.md) — Release-Historie
- [Contributing](../../CONTRIBUTING.md) — Wie Sie beitragen können
- [Security](../../SECURITY.md) — Schwachstellen melden
- [Issues](https://github.com/epicsagas/epic-harness/issues) — Fehlerberichte und Feature-Wünsche

## Danksagung

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Automatisierte Evolution und Benchmark-Muster
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code Agent-Skill-System
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Umfassende Claude Code-Muster
- [gstack](https://github.com/garrytan/gstack) — Plugin-Architektur-Referenz
- [harness](https://github.com/revfactory/harness) — Hook- und Harness-Infrastrukturmuster
- [serena](https://github.com/oraios/serena) — Autonomes Agent-Design
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Multi-Befehl-Framework-Architektur
- [superpowers](https://github.com/obra/superpowers) — Claude Code-Erweiterungsmuster

## Lizenz

[Apache 2.0](../../LICENSE)
