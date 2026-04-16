# epic harness

**6 Befehle. Automatisch ausgelöste Skills. Selbstentwickelnd.**

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

Ein Claude Code Plugin, das **30+ Befehle durch 6 ersetzt**, **Skills automatisch auslöst** basierend auf dem aktuellen Kontext und **neue Skills entwickelt** aus eigenen Fehlermustern. Weniger Oberfläche zum Merken. Mehr Intelligenz pro Tastendruck.

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
  Werkzeugnutzung beobachten → Fehler analysieren → Skills automatisch generieren → prüfen → neu laden
```

## Installation

```
# Claude Code Plugin (empfohlen)
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# Oder aus dem Quellcode
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### Aus dem Binär installieren

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# Von crates.io
cargo install epic-harness

# Vorkompiliertes Binär (schneller, kein Kompilieren)
cargo binstall epic-harness

# Aus dem Quellcode
cargo install --path .
```

Das Binär wird automatisch von den Hooks erkannt. Falls es fehlt, fallen die Hooks auf Node.js zurück.

## Multi-Tool-Unterstützung

epic-harness funktioniert mit Claude Code und 6 weiteren KI-Coding-Tools. Alle Tools teilen dasselbe `~/.harness/projects/{slug}/`-Datenverzeichnis.

| Tool | Ring 0 Hooks | Befehle/Prompts | Skills | Agents |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Vollständig | ✓ 6 Befehle | ✓ 8 Skills | ✓ 4 |
| **Codex CLI** | ✓ Vollständig¹ | ✓ 6 Prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Teilweise² | ✓ 6 Befehle | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Vollständig³ | ✓ 6 Befehle | ✓ via Regeln | ✓ 4 |
| **OpenCode** | ✓ Teilweise⁴ | ✓ 6 Befehle | — | ✓ 4 |
| **Cline** | ✓ Vollständig⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Erfordert `codex_hooks = true` in `~/.codex/config.toml`; PostToolUse fängt nur Bash ab
² Kein `PreToolUse`-Äquivalent — guard läuft auf `BeforeModel`-Ebene
³ Erfordert Cursor 1.7+
⁴ JS-Plugin: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel Hook-Skripte
⁶ Kein Hook-System — Konventionen über `.aider/CONVENTIONS.md` + `.aider.conf.yml` injiziert

### Integration für andere Tools installieren

```bash
# Interaktives Menü (Tools zum Installieren auswählen)
epic install

# Direkte Installation
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (erfordert Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Projektlokal installieren
epic install cursor --local

# Vorschau ohne Änderungen
epic install gemini --dry-run
```

Integrationsdateien im Tool-Verzeichnis (`hooks.json`, Befehle, Agents, Skills, Regeln, …) werden vom Binär **synchronisiert**: fehlende oder veraltete Dateien werden geschrieben. `GEMINI.md` und `AGENTS.md` werden nur erstellt, wenn sie fehlen.

## Einheitlicher Speicher

Alle Agents teilen sich einen einzigen Wissensgraphen, gespeichert in `~/.harness/memory.db` (SQLite + FTS5). Kein Node.js oder externe Runtime erforderlich.

### Smart Recall

Der Speicherabruf verwendet **composite scoring** statt einfach die letzten N Einträge zu dumpen:

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **Wichtigkeit** automatisch nach Knotentyp gesetzt: decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **Zugriffsverfolgung**: häufig abgerufene Erinnerungen schwimmen natürlich nach oben
- **Allmählicher Zerfall**: ungenutzte Erinnerungen verlieren mit der Zeit an Wichtigkeit (10% alle 30 Tage, Boden 0.05)
- **Graph-Augmentierung**: Recall folgt 1-Hop-Kanten, um verwandten Kontext zu finden

### CLI

```bash
# Smart Recall — nach Relevanz für deine aktuelle Aufgabe eingestuft
harness mem recall "auth refactor" --project my-project

# Speicherknoten hinzufügen (Wichtigkeit automatisch nach Typ, oder explizit)
harness mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
harness mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# Filterabfrage (enthält Wichtigkeit + access_count)
harness mem query --type decision --project my-project

# Volltextsuche (nach Wichtigkeit eingestuft)
harness mem search "JWT"

# Smart Context (wichtigkeitsgewichtet, nicht nur neueste)
harness mem context --project my-project

# Wissensgraph-Web-UI
harness mem serve          # → http://localhost:7700

# Als MCP-Server in Claude Code registrieren (kein Node.js benötigt)
harness mem mcp-install

# Alle Knoten als Markdown exportieren für Git-Backup
harness mem export --out ./docs/memory
```

### MCP-Tools (6)

Wenn als MCP-Server registriert (`harness mem mcp-install`), können Agents diese Tools direkt aufrufen:

| Tool | Zweck |
|------|---------|
| `mem_recall` | **Primär.** Intelligenter kontextueller Recall mit Hint + Projekt + Graph-Nachbarn |
| `mem_add` | Knoten mit Auto-Wichtigkeit nach Typ hinzufügen (oder explizit 0.0–1.0) |
| `mem_search` | FTS5-Schlüsselwortsuche, Ergebnisse nach Wichtigkeit eingestuft |
| `mem_query` | Nach Tag/Typ/Projekt filtern |
| `mem_context` | Projektbezogener Smart Recall (kein Hint) |
| `mem_related` | BFS-Graphdurchlauf von einer Knoten-ID |

### Wie der Wissensgraph funktioniert

Der Graph akkumuliert sich automatisch aus der normalen Sitzungsarbeit — keine manuelle Eingabe erforderlich.

**Datenfluss:**

```
PostToolUse hook → observe (3-Achsen-Bewertung) → obs/*.jsonl
                                                         ↓
SessionEnd hook → reflect (Mustererkennung) → memory.db Knoten + Kanten
                                                         ↓  (Wichtigkeit nach Typ gesetzt)
SessionStart hook → resume (Smart Recall) → nächste Sitzung erhält relevanzgeordnete Hinweise
                              ↓
                    decay_importance() → ungenutzte Knoten verblassen allmählich
```

**Knotentypen (7):**

| Typ | Erstellt durch | Standard-Wichtigkeit |
|------|-----------|-------------------|
| `decision` | Manuell / MCP | 0.9 |
| `resolution` | Manuell / MCP | 0.8 |
| `concept` | Manuell / MCP | 0.7 |
| `project` | Manuell / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**Speicher-Lebenszyklus:**

| Ereignis | Was passiert |
|-------|-------------|
| Knoten via Suche/Recall/Kontext abgerufen | `access_count++`, `accessed_at` aktualisiert |
| 30+ Tage ohne Zugriff | Wichtigkeit um 10% zerfallen (Boden 0.05) |
| 180+ Tage ohne Zugriff | als `stale` markiert, vom Recall ausgeschlossen |
| Knoten mit `pinned` markiert | immun gegen Zerfall |

**Bedingungen für automatische Akkumulation:**

| Bedingung | Erstellter Knoten |
|-----------|-------------|
| Jedes Sitzungsende | `session` (immer) |
| Gleicher Fehler ≥3 Mal hintereinander | `error` (repeated_same_error) |
| Edit→Error abwechselnd | `pattern` (thrashing) |
| Tool-Erfolgsrate <60% (min. 5 Beobachtungen) | `pattern` (weak_tool) |
| Dateityp-Erfolgsrate <50% (min. 3 Beobachtungen) | `pattern` (weak_filetype) |
| Edit-Erfolg → Bash-Fehler-Zyklen | `pattern` (fix_then_break) |

> **Hinweis:** Saubere Sitzungen (keine Fehler) erzeugen nur `session`-Knoten. Der Graph wird nach 2–3 echten Entwicklungssitzungen mit Build-Fehlern, Testfehlern oder Debugging-Zyklen reichhaltig.

Bestehende dateibasierte Erinnerungen (`nodes/*.md`, `edges.jsonl`) werden beim ersten Start automatisch nach SQLite migriert.

## Befehle

| Befehl | Beschreibung |
|--------|-------------|
| `/spec` | Definiere, was gebaut werden soll — Anforderungen klären, Spezifikation erstellen |
| `/go` | Bauen — automatische Planung, TDD-Subagenten, parallele Ausführung |
| `/check` | Prüfen — paralleles Code-Review + Sicherheitsaudit + Performance-Analyse |
| `/ship` | Ausliefern — PR, CI, Merge |
| `/team` | Org-level-Agenten-Teams projektübergreifend erstellen und synchronisieren |
| `/evolve` | Manuelle Evolution auslösen / Status / Rollback |

## Teams (`epic team`)

Teams sind **org-level**, nicht projektgebunden. Das Ausführen von `/team` in einem beliebigen Projekt bereichert einen gemeinsamen Pool von Agent-Definitionen — überschreibt nie stillschweigend.

### Funktionsweise

```
epic team                      # interaktiv: Projekt scannen → entwerfen → schreiben → synchronisieren
         ↓
~/.harness/orgs/epic/teams/backend/   ← globaler Speicher (projektübergreifend persistent)
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code entdeckt automatisch beim Sitzungsstart
├── domain-expert.md                  ← Rollendefinition + Team-Kontext injiziert
├── reviewer.md
└── tester.md
         ↓
Nächste Sitzung: Agents aktiv — automatisch von Claude ausgewählt oder explizit aufgerufen
```

### CLI-Referenz

```bash
# Team erstellen oder aktualisieren (interaktiver 4-Phasen-Ablauf)
epic team

# Durchsuchen
epic team list                        # alle Teams im aktuellen Org
epic team list --org netflix          # Teams in einem benannten Org
epic team show backend                # Konfiguration, Mission, Agents
epic team show backend --playbook     # + vollständiges angesammeltes Playbook

# Zum Projekt deployen
epic team sync backend                # deploy: Agents kopieren → .claude/agents/backend/
epic team link backend                # deploy + Projekt in Team-Konfiguration registrieren

# Aus Projekt zurückrufen
epic team delete backend              # zurückrufen: nur aus aktuellem Projekt entfernen
epic team unlink backend              # Alias für delete

# Auflösen (vollständig aus Org entfernen)
epic team delete backend --global     # dauerhaft aus Org-Speicher + lokaler Kopie löschen

# Verlauf
epic team history backend reviewer    # .history/-Backups für einen Agent auflisten
```

### Teams aus Coding-Agents verwenden

Nach der Synchronisierung sind Agents automatisch in der nächsten Sitzung verfügbar:

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert das Zahlungsgateway implementieren
@reviewer diesen PR auf Randfälle prüfen
@tester Integrationstests für auth schreiben

# Oder den Agent basierend auf dem Aufgabenkontext automatisch auswählen lassen
```

Jede Agent-Datei trägt einen **Team-Kontext**-Abschnitt, der bei der Synchronisierung injiziert wird:

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

Agents kennen ihr Team, ihre Mission und wie das vollständige Playbook bei Bedarf geladen wird —
ohne das Kontextfenster damit aufzublähen.

### Multi-Org

```bash
epic team                          # akkumuliert im "epic"-Org (Standard)
epic team --org netflix            # separate Netflix-Topologie
epic team --org client-x           # pro-Client-Engagement
```

Gleicher Teamname im gleichen Org = absichtliches projektübergreifendes Teilen.
`epic/teams/backend` akkumuliert Wissen aus jedem Projekt, das es erstellt oder verlinkt.

### Team-Typen

| Typ | Schlüsselwort | Standard-Agents |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### Merge-Strategie — keine stillen Überschreibungen

| Objekt | Regel |
|--------|------|
| Agent — neu | Automatisch hinzufügen |
| Agent — unverändert | Überspringen |
| Agent — geändert | **Aufforderung** (Standard: vorhandene behalten). Bei Ersatz → in `.history/` gesichert |
| `playbook.md` | Immer **anhängen** — nie abschneiden |
| `mission.md` — geändert | **Aufforderung** (Standard: vorhandene behalten) |

## Auto Skills (Ring 2)

Skills werden automatisch basierend auf dem Kontext ausgelöst. Du musst sie nicht manuell aufrufen.

| Skill | Wird ausgelöst, wenn |
|-------|----------------------|
| **tdd** | Neue Feature-Implementierung |
| **debug** | Testfehler oder Laufzeitfehler |
| **secure** | Auth/DB/API/Secrets-Code berührt wird |
| **perf** | Schleifen, Abfragen, Rendering-Code |
| **simplify** | Datei > 200 Zeilen oder hohe Komplexität |
| **document** | Öffentliche API hinzugefügt oder geändert |
| **verify** | Vor dem Abschluss von /go oder /ship |
| **context** | Kontextfenster > 70% belegt |

## Hooks (Ring 0)

Laufen unsichtbar. Keine Benutzeraktion erforderlich. Implementiert als **einzelne Rust-Binary** (`epic-harness`) mit Unterbefehlen, mit Fallback auf Node.js falls das Binär fehlt.

```
epic resume | guard | polish | observe | snapshot | reflect
```

| Hook | Wann | Funktion |
|------|------|----------|
| **resume** | Sitzungsstart | Kontext wiederherstellen, Speicher laden, Stack erkennen |
| **guard** | Vor Bash | Force-Push-auf-Main, rm -rf /, DROP prod blockieren |
| **polish** | Nach Edit | Auto-Formatierung (Biome/Prettier/ruff/gofmt) + Typprüfung |
| **observe** | Bei jeder Werkzeugnutzung | Protokollierung in `~/.harness/projects/{slug}/obs/` für Evolution |
| **snapshot** | Vor Komprimierung | Zustand in `~/.harness/projects/{slug}/sessions/` speichern |
| **reflect** | Sitzungsende | Fehler analysieren, evolvierte Skills erzeugen, prüfen |

## Eval-System (Ring 3 Kern)

Verschmilzt die Benchmark-Muster von A-Evolve mit dem Hook-System von Claude Code.

### Mehrdimensionale Bewertung

Jeder Werkzeugaufruf wird auf 3 Achsen bewertet. Gewichte sind konfigurierbar über `SCORE_WEIGHTS` in `src/hooks/common.rs`:

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (Standard: 0.5)                        (Standard: 0.3)                           (Standard: 0.2)
```

| Dimension | Was sie misst | Pro-Werkzeug-Kriterien |
|-----------|--------------|------------------------|
| `tool_success` | Hat es funktioniert? (0/1) | 9-Kategorien-Fehlerklassifikation |
| `output_quality` | Ausgabequalitätssignale (0.0-1.0) | Bash: Warnungen, leere Ausgabe. Edit: Erneutes-Bearbeiten-Erkennung |
| `execution_cost` | Effizienz-Proxy (0.0-1.0) | Ausgabegröße, Whitelist für stille Erfolgsbefehle |

### Fehlerklassifikation (9 Kategorien)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Mustererkennung (4 Typen)

Alle Schwellenwerte sind konfigurierbare Konstanten in `src/hooks/common.rs`:

| Muster | Erkennt | Konstante | Standard |
|--------|---------|-----------|----------|
| `repeated_same_error` | Gleicher Fehler N+ Mal hintereinander | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edit erfolgreich → Build/Test schlägt fehl | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Festgefahren an derselben Datei N+ Operationen | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Edit↔Error abwechselnd an derselben Datei | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Skill-Seeding-Schwellenwerte

| Auslöser | Konstante | Standard |
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
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: pro Werkzeug, pro Dateierweiterung, Score-Verteilung
    ↓ Muster: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 Pfade: Muster / schwaches Werkzeug / schwacher Dateityp / hochfrequenter Fehler)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate (Formatprüfung, Duplikaterkennung, Obergrenze von 10, Stagnationsprüfung)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (bester Checkpoint)
Reload (nächste Sitzung — resume.ts meldet Metriken + lädt evolvierte Skills)
```

```bash
/evolve              # Evolution jetzt ausführen
/evolve status       # Dashboard: Scores, Trends, Muster, Skills
/evolve history      # Langzeitanalyse: vollständige Historie, Skill-Effektivität, Dispatch-Statistiken
/evolve cross-project # Projektübergreifende Musteranalyse
/evolve rollback     # Vorherigen besten Zustand wiederherstellen
/evolve reset        # Alle Evolutionsdaten löschen
```

## Kaltstart-Voreinstellungen

Es ist nicht nötig, 5 Sitzungen auf nützliche evolvierte Skills zu warten. Bei der ersten Sitzung erkennt epic harness deinen Stack und wendet automatisch voreingestellte Skills an:

| Stack | Voreingestellte Skills |
|-------|----------------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Voreinstellungen sind Ergänzungen — sie werden durch echte evolvierte Skills ersetzt, sobald genügend Daten vorliegen.

## Sicherheit bei parallelen Sitzungen

Jede Sitzung schreibt in ihre eigene Beobachtungsdatei (`session_{date}_{pid}_{random}.jsonl`). Mehrere Claude Code Sitzungen im selben Projekt beschädigen nicht gegenseitig ihre Daten. Der reflect-Hook führt alle Dateien desselben Tages für die Analyse zusammen.

## Benutzerdefinierte Schutzregeln

Füge projektspezifische Sicherheitsregeln über `.harness/guard-rules.yaml` im Projektstamm hinzu:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Regeln werden mit den eingebauten Schutzregeln zusammengeführt (Force-Push-auf-Main, rm -rf /, DROP prod). Diese Datei in git zu pflegen ermöglicht das Teilen von Sicherheitsregeln mit deinem Team.

## Projektübergreifendes Lernen

Opt-in zum Teilen von Fehlermustern zwischen Projekten:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # Opt-in
```

Wenn aktiviert:
- Sitzungsende exportiert anonymisierte Muster nach `~/.harness/global_patterns.jsonl`
- Sitzungsstart zeigt Hinweise aus Schwachstellen anderer Projekte
- Nutze `/evolve cross-project` für aggregierte Musteransicht

## Skill-Effektivitätsverfolgung

Jeder evolvierte Skill wird mit A/B-Attributionswerten verfolgt:

```
/evolve history → Abschnitt Skill-Effektivität

| Skill              | Sitzungen | Score mit | Score ohne | Delta  |
|--------------------|-----------|-----------|------------|--------|
| evo-ts-care        | 8         | 0.87      | 0.72       | +15%   |
| evo-bash-discipline| 3         | 0.65      | 0.68       | -3%    |
```

Positives Delta = Skill hilft. Negatives Delta = Entfernung via `/evolve rollback` in Betracht ziehen.

## Polish → Observe Rückkopplung

Der polish-Hook (Auto-Formatierung + Typprüfung) speist Ergebnisse zurück in die Beobachtungspipeline:

- Formatierungsfehler → als `lint_fail` erfasst
- TypeScript-Fehler → als `build_fail` erfasst
- Erfolge → mit vollständigen Scores erfasst

Das bedeutet, dass "Edit → Typfehler → Edit → Typfehler"-Thrashing-Muster erkannt werden, auch wenn die Fehler vom polish-Hook stammen und nicht von manuellen Befehlen.

## Projektdaten (`~/.harness/projects/{slug}/`)

Projektspezifische Daten liegen in deinem Home-Verzeichnis. Sie überleben Projektlöschungen und belasten nicht die Git-Historie.

```
~/.harness/projects/{slug}/
├── memory/           # Projektmuster und Regeln (persistent)
├── sessions/         # Sitzungs-Snapshots (für Wiederherstellung)
├── obs/              # Werkzeugnutzungs-Beobachtungsprotokolle (JSONL, pro Sitzung)
├── evolved/          # Automatisch evolvierte Skills
├── evolved_backup/   # Bester Checkpoint (für Stagnations-Rollback)
├── dispatch/         # Skill-Dispatch-Protokolle (JSONL)
├── team/             # legacy (abgelöst durch ~/.harness/orgs/)
├── evolution.jsonl   # Vollständige Evolutionshistorie
└── metrics.json      # Aggregierte Statistiken + Skill-Attribution

~/.harness/
├── memory.db         # SQLite-Wissensgraph (Knoten + Kanten + FTS5)
├── graph.json        # Gecachter Graph (für Web-UI)
└── orgs/             # epic team globaler Speicher
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

Du kannst weiterhin `.harness/guard-rules.yaml` im Projektstamm verwenden, um Sicherheitsregeln mit deinem Team zu teilen.

## Entwicklung

### Build

```bash
cargo install --path .          # Bauen + installieren nach ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Plugin-Binary aktualisieren
```

### Wie Hooks dispatcht werden

Jeder Hook in `hooks.json` sucht die Rust-Binary an zwei Stellen:

```
1. Plugin lokal: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (via cargo install)
```

### Tests

```bash
cargo test       # Rust Unit- + Integrationstests
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
