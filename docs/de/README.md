# epic harness

**6 Befehle. Automatisch ausgeloeste Skills. Selbstentwickelnd.**

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

Ein Claude Code Plugin, das **30+ Befehle durch 6 ersetzt**, **Skills automatisch ausloest** basierend auf dem aktuellen Kontext und **neue Skills entwickelt** aus eigenen Fehlermustern. Weniger Oberflaeche zum Merken. Mehr Intelligenz pro Tastendruck.

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness Features" width="100%" />
</p>

## Architektur: 4-Ring-Modell

```
Ring 0 — Autopilot (Hooks, unsichtbar)
  Sitzungswiederherstellung, Auto-Formatierung, Sicherheitsschranken, Beobachtungsprotokollierung

Ring 1 — 6 Befehle (diese rufst du auf)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — Auto Skills (kontextgesteuert)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — Evolve (selbstverbessernd)
  Werkzeugnutzung beobachten → Fehler analysieren → Skills automatisch generieren → pruefen → neu laden
```

## Installation

```bash
# Claude Code Plugin Marketplace
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# Oder manuell
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rust-Binary (optional, ~4x schnellere Hooks)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# Von crates.io
cargo install epic-harness
# oder mit cargo-binstall (vorkompiliert, schneller)
cargo binstall epic-harness

# Aus dem Quellcode
cargo install --path .
```

Die Binary wird automatisch von den Hooks erkannt. Falls nicht vorhanden, wird auf Node.js zurueckgegriffen.

## Befehle

| Befehl | Beschreibung |
|--------|-------------|
| `/spec` | Definiere, was gebaut werden soll — Anforderungen klaeren, Spezifikation erstellen |
| `/go` | Bauen — automatische Planung, TDD-Subagenten, parallele Ausfuehrung |
| `/check` | Pruefen — paralleles Code-Review + Sicherheitsaudit + Performance-Analyse |
| `/ship` | Ausliefern — PR, CI, Merge |
| `/team` | Projektspezifisches Agenten-Team entwerfen |
| `/evolve` | Manuelle Evolution ausloesen / Status / Rollback |

## Auto Skills (Ring 2)

Skills werden automatisch basierend auf dem Kontext ausgeloest. Du musst sie nicht manuell aufrufen.

| Skill | Wird ausgeloest, wenn |
|-------|----------------------|
| **tdd** | Neue Feature-Implementierung |
| **debug** | Testfehler oder Laufzeitfehler |
| **secure** | Auth/DB/API/Secrets-Code beruehrt wird |
| **perf** | Schleifen, Abfragen, Rendering-Code |
| **simplify** | Datei > 200 Zeilen oder hohe Komplexitaet |
| **document** | Oeffentliche API hinzugefuegt oder geaendert |
| **verify** | Vor dem Abschluss von /go oder /ship |
| **context** | Kontextfenster > 70% belegt |

## Hooks (Ring 0)

Laufen unsichtbar. Keine Benutzeraktion erforderlich. Implementiert als **einzelne Rust-Binary** (`epic-harness`) mit Unterbefehlen, mit Fallback auf Node.js, falls die Binary nicht verfuegbar ist.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| Hook | Wann | Funktion |
|------|------|----------|
| **resume** | Sitzungsstart | Kontext wiederherstellen, Speicher laden, Stack erkennen |
| **guard** | Vor Bash | Force-Push-auf-Main, rm -rf /, DROP prod blockieren |
| **polish** | Nach Edit | Auto-Formatierung (Biome/Prettier/ruff/gofmt) + Typenpruefung |
| **observe** | Bei jeder Werkzeugnutzung | Protokollierung in `.harness/obs/` fuer Evolution |
| **snapshot** | Vor Komprimierung | Zustand in `.harness/sessions/` speichern |
| **reflect** | Sitzungsende | Fehler analysieren, evolvierte Skills erzeugen, pruefen |

## Eval-System (Ring 3 Kern)

Verschmilzt die Benchmark-Muster von A-Evolve mit dem Hook-System von Claude Code.

### Mehrdimensionale Bewertung

Jeder Werkzeugaufruf wird auf 3 Achsen bewertet. Gewichte sind konfigurierbar ueber `SCORE_WEIGHTS` in `src/ts/common.ts` (oder `src/hooks/common.rs`):

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (Standard: 0.5)                        (Standard: 0.3)                           (Standard: 0.2)
```

| Dimension | Was sie misst | Pro-Werkzeug-Kriterien |
|-----------|--------------|------------------------|
| `tool_success` | Hat es funktioniert? (0/1) | 9-Kategorien-Fehlerklassifikation |
| `output_quality` | Ausgabequalitaetssignale (0.0-1.0) | Bash: Warnungen, leere Ausgabe. Edit: Erneutes-Bearbeiten-Erkennung |
| `execution_cost` | Effizienz-Proxy (0.0-1.0) | Ausgabegroesse, Whitelist fuer stille Erfolgsbefehle |

### Fehlerklassifikation (9 Kategorien)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Mustererkennung (4 Typen)

Alle Schwellenwerte sind konfigurierbare Konstanten in `src/ts/common.ts` (oder `src/hooks/common.rs`):

| Muster | Erkennt | Konstante | Standard |
|--------|---------|-----------|----------|
| `repeated_same_error` | Gleicher Fehler N+ Mal hintereinander | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edit erfolgreich → Build/Test schlaegt fehl | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Festgefahren an derselben Datei N+ Operationen | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Edit↔Error abwechselnd an derselben Datei | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Skill-Seeding-Schwellenwerte

| Ausloeser | Konstante | Standard |
|-----------|-----------|----------|
| Schwaches Werkzeug (niedrige Erfolgsrate) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| Schwacher Dateityp | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| Hochfrequenter Fehler | `HIGH_FREQ_ERROR_MIN` | 5 |

### Stagnations-Gating

- `STAGNATION_LIMIT` (Standard: 3) Sitzungen ohne Verbesserung → automatischer Rollback evolvierter Skills auf den besten Checkpoint
- `IMPROVEMENT_THRESHOLD` (Standard: 5%)
- Trendverfolgung: `improving` / `stable` / `declining` via lineare Regression
- Statische Skills haben bei Konflikten immer Vorrang vor evolvierten Skills

### Evolutionsfluss

```
Observe (PostToolUse — 3-Achsen-Bewertung)
    ↓ .harness/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: pro Werkzeug, pro Dateierweiterung, Score-Verteilung
    ↓ Muster: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 Pfade: Muster / schwaches Werkzeug / schwacher Dateityp / hochfrequenter Fehler)
    ↓ .harness/evolved/{skill}/SKILL.md
Gate (Formatpruefung, Duplikaterkennung, Obergrenze von 10, Stagnationspruefung)
    ↓ .harness/evolved_backup/ (bester Checkpoint)
Reload (naechste Sitzung — resume.ts meldet Metriken + laedt evolvierte Skills)
```

```bash
/evolve              # Evolution jetzt ausfuehren
/evolve status       # Dashboard: Scores, Trends, Muster, Skills
/evolve history      # Langzeitanalyse: vollstaendige Historie, Skill-Effektivitaet, Dispatch-Statistiken
/evolve cross-project # Projektuebergreifende Musteranalyse
/evolve rollback     # Vorherigen besten Zustand wiederherstellen
/evolve reset        # Alle Evolutionsdaten loeschen
```

## Kaltstart-Voreinstellungen

Es ist nicht noetig, 5 Sitzungen auf nuetzliche evolvierte Skills zu warten. Bei der ersten Sitzung erkennt epic harness deinen Stack und wendet automatisch voreingestellte Skills an:

| Stack | Voreingestellte Skills |
|-------|----------------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Voreinstellungen sind Ergaenzungen — sie werden durch echte evolvierte Skills ersetzt, sobald genuegend Daten vorliegen.

## Sicherheit bei parallelen Sitzungen

Jede Sitzung schreibt in ihre eigene Beobachtungsdatei (`session_{date}_{pid}_{random}.jsonl`). Mehrere Claude Code Sitzungen im selben Projekt beschaedigen nicht gegenseitig ihre Daten. Der reflect-Hook fuehrt alle Dateien desselben Tages fuer die Analyse zusammen.

## Benutzerdefinierte Schutzregeln

Fuege projektspezifische Sicherheitsregeln ueber `.harness/guard-rules.yaml` hinzu:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Regeln werden mit den eingebauten Schutzregeln zusammengefuehrt (Force-Push-auf-Main, rm -rf /, DROP prod).

## Projektuebergreifendes Lernen

Opt-in zum Teilen von Fehlermustern zwischen Projekten:

```bash
touch .harness/.cross-project-enabled  # Opt-in
```

Wenn aktiviert:
- Sitzungsende exportiert anonymisierte Muster nach `~/.harness-global/patterns.jsonl`
- Sitzungsstart zeigt Hinweise aus Schwachstellen anderer Projekte
- Nutze `/evolve cross-project` fuer aggregierte Musteransicht

## Skill-Effektivitaetsverfolgung

Jeder evolvierte Skill wird mit A/B-Attributionswerten verfolgt:

```
/evolve history → Abschnitt Skill-Effektivitaet

| Skill              | Sitzungen | Score mit | Score ohne | Delta  |
|--------------------|-----------|-----------|------------|--------|
| evo-ts-care        | 8         | 0.87      | 0.72       | +15%   |
| evo-bash-discipline| 3         | 0.65      | 0.68       | -3%    |
```

Positives Delta = Skill hilft. Negatives Delta = Entfernung via `/evolve rollback` in Betracht ziehen.

## Polish → Observe Rueckkopplung

Der polish-Hook (Auto-Formatierung + Typenpruefung) speist Ergebnisse zurueck in die Beobachtungspipeline:

- Formatierungsfehler → als `lint_fail` erfasst
- TypeScript-Fehler → als `build_fail` erfasst
- Erfolge → mit vollstaendigen Scores erfasst

Das bedeutet, dass "Edit → Typfehler → Edit → Typfehler"-Thrashing-Muster erkannt werden, auch wenn die Fehler vom polish-Hook stammen und nicht von manuellen Befehlen.

## Projektdaten (`.harness/`)

epic harness erstellt ein `.harness/`-Verzeichnis in deinem Projekt:

```
.harness/
├── memory/           # Projektmuster und Regeln (persistent)
├── sessions/         # Sitzungs-Snapshots (fuer Wiederherstellung)
├── obs/              # Werkzeugnutzungs-Beobachtungsprotokolle (JSONL, pro Sitzung)
├── evolved/          # Automatisch evolvierte Skills
├── evolved_backup/   # Bester Checkpoint (fuer Stagnations-Rollback)
├── dispatch/         # Skill-Dispatch-Protokolle (JSONL)
├── team/             # Von /team generierte Agenten und Skills
├── evolution.jsonl   # Vollstaendige Evolutionshistorie
├── metrics.json      # Aggregierte Statistiken + Skill-Attribution
└── guard-rules.yaml  # Benutzerdefinierte Schutzregeln (optional)
```

Fuege `.harness/` zu `.gitignore` hinzu oder committe es — deine Entscheidung.

## Entwicklung

### Rust (primaer — ~4x schneller)

```bash
cargo install --path .          # Bauen + installieren nach ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Plugin-Binary aktualisieren
```

### Node.js (Fallback)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### Wie Hooks dispatcht werden

Jeder Hook in `hooks.json` sucht die Rust-Binary an drei Stellen und faellt dann auf Node.js zurueck:

```
1. Plugin lokal: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (via cargo install)
3. Fallback:     node hooks/scripts/<hook>.js
```

### Tests

```bash
cargo test       # 98 Rust-Unit-Tests
npm test         # Node.js Unit- + E2E-Tests
```

## Danksagungen

epic harness wurde inspiriert von und aufgebaut auf Ideen folgender Projekte:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Automatisierte Evolution und Benchmark-Muster
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code Agent-Skill-System
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Umfassende Claude Code Muster
- [gstack](https://github.com/garrytan/gstack) — Plugin-Architektur-Referenz
- [harness](https://github.com/revfactory/harness) — Hook- und Harness-Infrastrukturmuster
- [serena](https://github.com/oraios/serena) — Autonomes Agenten-Design
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Multi-Command-Framework-Architektur
- [superpowers](https://github.com/obra/superpowers) — Claude Code Erweiterungsmuster

## Lizenz

[Apache 2.0](LICENSE)
