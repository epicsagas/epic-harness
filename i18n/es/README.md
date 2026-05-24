<h1 align="center">Epic Harness</h1>

<blockquote><p align="center">Un arnés de agente IA multiherramienta que aprende de cada sesión — 22 skills, pipelines autónomos y motor autoevolutivo.</p></blockquote>

<p align="center"><b>Un arnés, seis herramientas IA. Autónomo del spec al PR. Más inteligente cada sesión.</b></p>

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
  <img alt="Version" src="https://img.shields.io/badge/version-0.4.4-fc8d62?style=for-the-badge&labelColor=0d1117" />
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.87+-d73a49?style=for-the-badge&labelColor=0d1117&logo=rust&logoColor=white" />
  <img alt="Tools" src="https://img.shields.io/badge/tools-6_supported-bc8cff?style=for-the-badge&labelColor=0d1117" />
  <a href="https://buymeacoffee.com/epicsaga"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/buy_me_a_coffee-FFDD00?style=for-the-badge&labelColor=0d1117&logo=buymeacoffee&logoColor=black" /></a>
</p>

Un arnés de agente IA multiherramienta con **22 skills (8 pipeline + 14 quality gates)**, un **motor autoevolutivo**, **memoria unificada** y una **pipeline autónoma de un solo comando** (`/orbit`). Funciona con Claude Code, Codex, Antigravity, Cursor, OpenCode y Cline — todos compartiendo el mismo directorio `~/.harness/`. Después de cada sesión, el bucle de evolución analiza fallos, genera skills dirigidos y los carga en la próxima sesión.

<p align="center">
  <img src="../../assets/features.png" alt="características de epic harness" width="100%" />
</p>

---

![Demo](../../docs/demo/demo.gif)

### Panel web — 10 pantallas con métricas en tiempo real
<p align="center">
  <img src="../../assets/dashboard.png" alt="Dashboard" width="49%" />
  <img src="../../assets/dashboard-orbit.png" alt="Orbit Pipeline" width="49%" />
</p>

---

## Qué hace

Un comando lleva una funcionalidad de extremo a extremo. Las habilidades se activan sin que tú las pidas. El agente se vuelve más inteligente después de cada sesión.

```bash
$ /orbit "Agregar autenticación JWT a la API de login"
→ spec approved → go (TDD subagents) → check (PASS) → ship (PR + CI) → evolve
```

O invoca las skills del pipeline directamente:

```bash
/spec "Agregar autenticación JWT a la API de login"   # aclara requisitos → SPEC-*.md
/go                                                     # planificación automática → subagentes TDD → 4 min
/check                                                  # revisión paralela + seguridad + pruebas → PASS
/ship                                                   # prueba aislada → PR → CI en verde
```

Las habilidades se activan automáticamente en segundo plano — sin comandos extra:

```
¿Construyendo una funcionalidad?   → tdd se activa (Red→Green→Refactor obligatorio)
¿Falló una prueba?                 → debug se activa (primero causa raíz, sin parches al azar)
¿Tocaste auth o BD?                → secure se activa (checklist OWASP, sin atajos)
¿Archivo con más de 200 líneas?    → simplify se activa (extraer, renombrar, simplificar)
```

Al cerrar la sesión, el **bucle evolve** analiza qué falló, genera habilidades enfocadas y las carga en la siguiente sesión. El agente que tuvo problemas con fallos de build de TypeScript tendrá una habilidad `evo-ts-care` la próxima vez.

---

## Instalación

> **¿Primera vez?** Lee la [Guía de inicio rápido (5 min)](../../docs/quickstart.md).

### Claude Code (recomendado)

```
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

Instala automáticamente el binario y registra todos los hooks en un solo paso.

### Antigravity

```bash
gemini plugin marketplace add epicsagas/plugins
```

Las habilidades y comandos están disponibles inmediatamente.

### macOS / Linux

```bash
brew install epicsagas/tap/epic-harness
```

¿No tienes Homebrew? Usa el script de instalación:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.ps1 | iex
```

### Vía toolchain de Rust

```bash
cargo binstall epic-harness   # binario precompilado (rápido)
cargo install epic-harness    # compilar desde el código fuente
```

Luego ejecuta el asistente de configuración:

```bash
epic install antigravity   # Antigravity
epic install cursor         # Cursor IDE
```

> `epic-harness --version` para verificar. Actualiza con `brew upgrade epic-harness` o vuelve a ejecutar el script de instalación.

Requisitos previos: **Git**. Las instalaciones desde el código fuente o binario también necesitan el [toolchain de Rust](https://rustup.rs).

### `epic install` — asistente de configuración

Después de instalar el binario, ejecuta `epic install` (o `epic install claude`) para:

1. Crear la estructura de directorios `~/.harness/`
2. Sincronizar comandos y habilidades al directorio de configuración de la herramienta
3. Registrar el servidor MCP (harness-mem) para Claude Code
4. Crear `~/.harness/config.toml` con valores predeterminados si no existe

En Claude Code, `hooks/install.js` se ejecuta automáticamente al inicio de la sesión e instala el binario si falta. No se necesita ningún paso manual después del clon inicial.

### Otras herramientas

```bash
epic install antigravity   # Antigravity    → ~/.gemini/config/plugins/epic/
epic install cursor         # Cursor         → ~/.cursor/ (requiere Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/
epic install              # Menú interactivo
```

Los archivos de integración se **sincronizan** desde el binario: los archivos faltantes u obsoletos se escriben. `GEMINI.md` y `AGENTS.md` solo se crean cuando están ausentes.

### Verificación

```bash
epic --version              # Binario instalado
ls ~/.harness/              # Directorio de datos existe
```

Dentro de una sesión de Claude Code: `/evolve status`

---

## Pipeline Skills (Ring 1)

8 habilidades que gestionan el ciclo de vida completo de desarrollo. Invocadas directamente o agrupadas automáticamente por `/orbit`.

| Habilidad | Qué hace |
|-----------|----------|
| **discover** | Reframing del problema — 5 Whys, JTBD, diálogo socrático |
| **spec** | Define requisitos — convierte a documento R + AC numerado |
| **go** | Fase de build — planificación automática → sub-agentes TDD → ejecución paralela → verificación AC |
| **check** | Fase de revisión — revisión de código paralela + auditoría de seguridad + tests con extras por ámbito |
| **ship** | Fase de entrega — test aislado → PR con informe completo → monitorización CI + auto-fix |
| **evolve** | Motor de auto-mejora — analizar sesiones, generar habilidades, métricas de efectividad, rollback |
| **/orbit** | Envoltorio autónomo — ejecuta discover → spec → go → check → ship → evolve en una sola invocación |
| **/team** | Explorar librerías de la organización, contratar equipos existentes o diseñar nuevos (3–6 agentes, sincronizados a `.claude/agents/`) |

Todas las habilidades del pipeline se activan automáticamente según el contexto o se pueden invocar por nombre. Los nombres antiguos de comandos siguen funcionando mediante enrutamiento de alias.

---

## /orbit — Pipeline Autónomo

`/orbit` envuelve todo el pipeline en una única ejecución autónoma. Elige un modo — todo lo demás es automático hasta el PR.

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

**Morado** — pasos humanos: selección de modo (requerimiento poco claro → interactivo), pausa por 3 fallos de check.
**Verde** — claro + complejo → council auto-spec; claro + simple → construcción directa; ambos completamente autónomos.

Estado persistido en `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` — sobrevive a la compactación del contexto.

> **Advertencias**: El agente puede omitir el pipeline cuando modifica orbit mismo o cuando solo edita documentación. Consulta [Problemas conocidos (Juicio del agente)](#problemas-conocidos-juicio-del-agente).

---

## Quality Gates (Ring 2)

Las habilidades se activan automáticamente según el contexto. No las invocas tú.

| Habilidad | Se activa cuando |
|-----------|-----------------|
| **tdd** | Implementación de nueva funcionalidad o corrección de errores |
| **debug** | Fallo de prueba o error en tiempo de ejecución |
| **secure** | Código de auth / BD / API / secrets modificado |
| **perf** | Bucles, consultas, renderizado, operaciones por lotes |
| **simplify** | Archivo > 200 líneas o alta complejidad ciclomática |
| **verify** | Antes de completar `/go` o `/ship` |
| **council** | Decisiones arquitectónicas o de diseño ambiguas |
| **document** | API pública añadida o firma modificada |
| **context** | Ventana de contexto > 70% |
| **orchestrate** | Estado de orquestación multi-agente y control de agentes en tiempo real |
| **commit** | Generación de Conventional Commits — creado automáticamente desde git diff |
| **agent-introspection** | 3+ fallos consecutivos o patrón de reintentos circular |
| **reflect** | Bajo demanda `/reflect`: autoevaluación humana — "¿Estoy usando la IA como amplificador del pensamiento?" 5 dimensiones con datos del hook |

---

## Evolve (Ring 3)

El arnés monitorea cada llamada de herramienta, la puntúa en 3 ejes, detecta patrones de fallo y genera habilidades enfocadas — automáticamente, al final de la sesión.

### Puntuación

```
composite = 0.5 × tool_success + 0.3 × output_quality + 0.2 × execution_cost
```

Clasificación de fallos (9 tipos): `type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Detección de Patrones

| Patrón | Detecta | Umbral predeterminado |
|--------|---------|----------------------|
| `repeated_same_error` | Mismo error N+ veces | 3 |
| `fix_then_break` | Éxito de edición → fallo de build/test | 3 lookahead, 2 ciclos |
| `long_debug_loop` | Atascado en el mismo archivo | 5 operaciones |
| `thrashing` | Alternancia Edit↔Error | 3 ediciones, 3 errores |

### Flujo de Evolución

```
Observe (PostToolUse — puntuación en 3 ejes)
    ↓ obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ puntuaciones por herramienta, por extensión + patrones
Propose (Solver — graduado por puntuación: ≥0.90 omitir, ≥0.70 moderado, <0.70 completo)
    ↓ SkillProposal[] con confianza
Curate (Aceptar/Fusionar/Omitir, retroalimentación enmascarada del solver)
    ↓ evolved/{skill}/SKILL.md + meta.json
Gate (verificación de formato, dedup, límite 10, promoción con puerta ≥ 3 sesiones)
    ↓ evolved_backup/ (mejor punto de control)
Instinct (patrones de alto éxito → nodos cross-project memory.db)
    ↓
Reload (siguiente sesión — resume carga habilidades evolucionadas)
```

Siembra de habilidades: herramienta débil (éxito <60%, mín. 5 obs), tipo de archivo débil (éxito <50%, mín. 3 obs), error de alta frecuencia (5+ ocurrencias).

Estancamiento: 3 sesiones sin una mejora del 5% → rollback automático al mejor punto de control.

### Efectividad de Habilidades

Cada habilidad evolucionada se rastrea con atribución A/B:

```
/evolve history → Skill Effectiveness

| Skill              | With | Without | Delta |
|--------------------|------|---------|-------|
| evo-ts-care        | 0.87 | 0.72    | +15%  |
| evo-bash-discipline| 0.65 | 0.68    | -3%   |
```

Delta positivo = efectiva. Negativo = considera eliminarla mediante `/evolve rollback`.

### Presets de Inicio en Frío

En la primera sesión, los presets de habilidades apropiados para el stack se aplican automáticamente:

| Stack | Presets |
|-------|---------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

### Aprendizaje de Instintos

Los patrones de alto éxito se extraen y promocionan entre proyectos:

```
observe (100% confirmado) → extract_instincts() → instinct node (confianza ≥ 0.8)
    → promover a global cuando se observe en ≥ 2 proyectos
```

```bash
/evolve              # Ejecutar ahora
/evolve status       # Panel: puntuaciones, tendencias, patrones, habilidades
/evolve history      # Historial completo + efectividad de habilidades
/evolve cross-project # Análisis de patrones entre proyectos
/evolve rollback     # Restaurar el mejor anterior
/evolve reset        # Borrar todos los datos de evolución
```

---

## Hooks (Ring 0)

Se ejecutan de forma invisible en cada sesión. Binario único de Rust (`epic-harness`) con subcomandos.

| Hook | Cuándo | Qué hace |
|------|--------|----------|
| **resume** | Inicio de sesión | Restaurar contexto, cargar memoria, detectar stack |
| **guard** | Antes de Bash | Bloquear force-push-to-main, `rm -rf /`, DROP prod |
| **polish** | Después de Edit | Autoformatear (Biome/Prettier/ruff/gofmt) + verificación de tipos |
| **observe** | Cada uso de herramienta | Registrar en `~/.harness/projects/{slug}/obs/` para evolución |
| **snapshot** | Antes de compactar | Guardar estado en `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Fin de sesión | Motor de evolución automática: análisis de fallos, seeding de skills, métricas, ingest a memoria. Alimenta `/reflect` |

Polish retroalimenta en observe: fallo de formato → `lint_fail`, error de TypeScript → `build_fail`. El thrashing Edit→Error se detecta incluso cuando los errores provienen de polish.

Cada sesión escribe su propio `session_{date}_{pid}_{random}.jsonl` — múltiples sesiones concurrentes no corromperán los datos de las demás.

### Perfiles de Hook

Mediante `~/.harness/config.toml` o la variable de entorno `EPIC_HOOK_PROFILE`:

| Perfil | Hooks activos |
|--------|--------------|
| `minimal` | guard, observe, resume |
| `standard` (predeterminado) | los anteriores + polish, reflect, snapshot |
| `strict` | todos los hooks + futuras verificaciones solo de strict |

### Reglas de Guard Personalizadas

Añade reglas específicas del proyecto mediante `.harness/guard-rules.yaml` en la raíz de tu proyecto:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

---

## Equipo (`epic team`)

Los equipos son de **nivel de organización**, no están vinculados al proyecto. Ejecutar `/team` en cualquier proyecto enriquece un grupo compartido de definiciones de agentes — nunca sobrescribe silenciosamente.

```bash
epic team                              # Interactivo: escanear → diseñar → escribir → sincronizar
epic team sync backend                 # Despachar agentes → .claude/agents/backend/
epic team link backend                 # Despachar + registrar proyecto en la configuración del equipo
epic team list                         # Todos los equipos en la organización actual
epic team list --org netflix           # Equipos en una organización nombrada
epic team show backend --playbook      # Configuración + playbook completo
epic team delete backend               # Retirar del proyecto actual solo
epic team delete backend --global      # Eliminar permanentemente del almacén de la organización
```

Después de sincronizar, los agentes están disponibles en la próxima sesión: `@domain-expert`, `@reviewer`, `@tester`, etc.

| Tipo | Palabra clave | Agentes predeterminados |
|------|--------------|------------------------|
| Alineado con el flujo | `stream` | domain-expert, reviewer, tester |
| Plataforma | `platform` | api-designer, infra-specialist, dx-agent |
| Habilitador | `enabling` | specialist |
| Subsistema complicado | `subsystem` | domain-specialist, integration-tester |

Multi-organización: `epic team --org netflix` — topología separada por organización.

Estrategia de fusión: los agentes modificados solicitan confirmación (predeterminado: mantener existente, respaldar en `.history/`). El playbook siempre se añade.

---

## Soporte Multi-Herramienta

Todas las herramientas comparten el mismo directorio de datos `~/.harness/projects/{slug}/`.

| Herramienta | Ring 0 Hooks | Habilidades | Agentes |
|-------------|-------------|-------------|---------|
| **Claude Code** | ✓ Completo | ✓ 22 habilidades | Live |
| **Codex CLI** | ✓ Completo¹ | ✓ 22 | — |
| **Antigravity** | ✓ Parcial² | ✓ 22 | — |
| **Cursor** | ✓ Completo³ | ✓ vía rules | Live |
| **OpenCode** | ✓ Parcial⁴ | — | — |
| **Cline** | ✓ Completo⁵ | — | — |
| **Aider** | —⁶ | — | — |

¹ Plugin marketplace · ² Guard al nivel `BeforeModel` · ³ Cursor 1.7+ · ⁴ Plugin JS · ⁵ 5 scripts de hook · ⁶ Solo convenciones

---

## Arquitectura: Modelo de 4 Anillos

```mermaid
flowchart TB
    subgraph R0["Ring 0 — Autopilot (hooks, invisible)"]
        direction LR
        h1(resume) --- h2(guard) --- h3(polish) --- h4(observe) --- h5(snapshot) --- h6(reflect)
    end

    subgraph R1["Ring 1 — Pipeline Skills (8)"]
        direction TB
        subgraph orbit_wrap["  /orbit  "]
            direction LR
            c1("discover") --> c2("spec") --> c3("go") --> c4("check") --> c5("ship") --> c6("evolve")
        end
        c7("/team")
        c8("/evolve (manual)")
    end

    subgraph R2["Ring 2 — Quality Gates (14, context-triggered)"]
        direction LR
        s1(tdd) --- s2(debug) --- s3(secure) --- s4(perf) --- s5(simplify) --- s6(verify) --- s7(council) --- s8(document) --- s9(context) --- s10(orchestrate) --- s11(commit) --- s12(agent-introspection) --- s13(reflect) --- s14(discover)
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

## Aprendizaje Entre Proyectos

Activa para compartir patrones de fallo entre proyectos:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled
```

Fin de sesión → exporta patrones anonimizados a `~/.harness/global_patterns.jsonl`. Inicio de sesión → muestra sugerencias de las áreas débiles de otros proyectos.

---

## Memoria Unificada

Todos los agentes comparten un grafo de conocimiento en `~/.harness/memory.db` (SQLite con búsqueda de texto completo). Sin runtime externo.

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

### CLI

```bash
epic mem recall "auth refactor" --project my-project   # Recuperación inteligente
epic mem add --title "JWT rotation" --type decision    # Añadir nodo
epic mem search "JWT"                                  # Búsqueda FTS5
epic mem list --type decision --project my-project    # Filtrar
epic mem context --project my-project                  # Contexto del proyecto
epic mem serve                                         # Interfaz web → :7700 o puerto personalizado con --port 8800
epic mem mcp-install                                   # Registrar servidor MCP
epic mem export --out ./docs/memory                    # Exportar a Markdown
```

### Herramientas MCP (6)

| Herramienta | Propósito |
|-------------|----------|
| `mem_recall` | Recuperación contextual inteligente con hint + project + vecinos del grafo |
| `mem_add` | Añadir nodo con importancia automática por tipo (o explícita 0.0–1.0) |
| `mem_search` | Búsqueda por palabra clave (texto completo), clasificada por importancia |
| `mem_query` | Filtrar por etiqueta/tipo/proyecto — alias de `mem_list` |
| `mem_context` | Recuperación inteligente con ámbito de proyecto (sin hint) |
| `mem_related` | Traversal del grafo desde un ID de nodo (encuentra conocimiento conectado) |

### Tipos de Nodo

| Tipo | Creado por | Importancia |
|------|-----------|-------------|
| `decision` | Manual / MCP | 0.9 |
| `resolution` | Manual / MCP | 0.8 |
| `concept` | Manual / MCP | 0.7 |
| `project` | Manual / MCP | 0.7 |
| `instinct` | Auto (reflect) | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

Ciclo de vida: más de 30 días sin acceso → 10% de decaimiento de importancia (mínimo 0.05). Más de 180 días → etiquetado como `stale`, excluido de la recuperación. La etiqueta `pinned` evita el decaimiento.

> **Interfaz web**: La visualización del grafo se está mejorando activamente — clustering, resaltado de vecinos y fallback offline se añadieron recientemente. Más mejoras en progreso.

---

<details>
<summary><strong>Datos del Proyecto — estructura de directorios</strong></summary>

## Datos del Proyecto

Todos los datos viven en `~/.harness/` (directorio home), no en la raíz de tu proyecto. Sobrevive a la eliminación del proyecto, no contamina el historial de git.

```
~/.harness/
├── memory.db                  # Grafo de conocimiento SQLite (nodos + aristas + FTS5)
├── graph.json                 # Grafo en caché (para la interfaz web)
├── config.toml                # Configuración del usuario
├── global_patterns.jsonl      # Patrones entre proyectos (opt-in)
├── orgs/                      # Almacén global del equipo
│   └── {org}/teams/{team}/
│       ├── config.json, mission.md, playbook.md, agents/, .history/
└── projects/{slug}/
    ├── memory/                # Patrones y reglas del proyecto
    ├── sessions/              # Snapshots de sesión (para resume)
    ├── obs/                   # Registros de observación de uso de herramientas (JSONL)
    ├── evolved/               # Habilidades autoevolucionadas
    │   ├── manifest.json
    │   └── {skill}/SKILL.md + meta.json
    ├── evolved_backup/        # Mejor punto de control (para rollback)
    ├── dispatch/              # Registros de despacho de habilidades
    ├── evolution.jsonl        # Historial completo de evolución
    └── metrics.json           # Estadísticas agregadas + atribución de habilidades
```

Comparte reglas de seguridad con tu equipo: `.harness/guard-rules.yaml` en la raíz del proyecto (comprometido en git).

</details>

---

<details>
<summary><strong>Configuración — referencia de config.toml</strong></summary>

## Configuración

Todos los parámetros ajustables en `~/.harness/config.toml`. Ausente = valores predeterminados en el código.

```toml
# Prioridad: variable de entorno (EPIC_HOOK_PROFILE) > este archivo > valores predeterminados

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

## Problemas conocidos (Juicio del agente)

Estos problemas surgen de la interpretación del contexto por parte del agente en lugar de errores en el código. Se listan aquí para que los usuarios sepan qué vigilar.

### Problemas descubiertos

| Problema | Cuándo | Qué sucede | Solución alternativa |
|----------|--------|------------|---------------------|
| **Omisión de autoduplicación en orbit** | Se pide a `/orbit` que mejore orbit mismo | El agente puede omitir el pipeline de orbit completamente y editar archivos ad-hoc en main, dejando cambios sin confirmar sin spec/PR/trazabilidad | Después de que orbit complete, verifica `git status`. Si hay cambios en main sin un estado de pipeline, confirma manualmente o vuelve a ejecutar `/orbit` desde una rama separada |
| **Tarea solo de docs omite el protocolo** | `/orbit` recibe un cambio solo de markdown (sin código para probar) | El agente puede juzgar las fases de TDD/test como innecesarias y omitir el pipeline completo | Aceptable para cambios puramente de documentación. Para código+docs mixtos, asegúrate de que el agente no omita las fases relacionadas con código |
| **Clasificación errónea de modo** | La solicitud está en el límite entre Direct y Council | El agente puede elegir Direct cuando Council (4 voces) capturaría más casos extremos, o Council cuando Direct bastaría | Si el agente elige un modo que no parece correcto, di "usa el modo Council" o "usa el modo Direct" explícitamente |

### Decisiones de diseño intencionales

Se consideraron para mejora pero se mantuvieron tal cual después de la evaluación:

| Decisión | Por qué no se mejoró | Fundamento |
|----------|---------------------|------------|
| **Worktree entra en la fase Go, no al inicio de orbit** | Podría aislar desde el preflight | Preflight/mode/spec son de solo lectura. Aislar antes añade complejidad sin beneficio — la rama no se crea hasta la fase Go de todos modos |
| **Worktree preservado después de Ship** | Podría eliminarse automáticamente al fusionar el PR | La rama es la cabeza del PR. Eliminarla antes de fusionar rompe el PR. La limpieza se deja al usuario después de la fusión |
| **Rama nombrada `orbit-{slug}` no `feature/{slug}`** | Podría coincidir con la nomenclatura convencional de ramas | `EnterWorktree` no permite `/` en los nombres. Renombrar post-creación añade un paso solo por beneficio cosmético |
| **Sin pipeline ligero para cambios de docs** | Podría detectar solo-docs y omitir TDD/tests | La detección es frágil (¿qué cuenta como "doc"?). Añadir una ruta separada aumenta la complejidad del protocolo para una ganancia marginal |

---

## Solución de problemas

<details>
<summary>command not found: epic después de instalar</summary>

Añade el directorio bin de Cargo a tu PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Añade esta línea a tu `~/.zshrc` o `~/.bashrc` para hacerlo permanente.
</details>

<details>
<summary>Los hooks no se ejecutan en Claude Code</summary>

Vuelve a ejecutar la instalación para sincronizar los hooks en la configuración de Claude Code:

```bash
epic install claude
```

Luego reinicia Claude Code. Los hooks se escriben en `~/.claude/settings.json`.
</details>

<details>
<summary>Permission denied en macOS (Gatekeeper)</summary>

macOS puede bloquear binarios sin firma descargados de internet:

```bash
xattr -d com.apple.quarantine ~/.cargo/bin/epic-harness
xattr -d com.apple.quarantine ~/.cargo/bin/epic
```
</details>

<details>
<summary>epic: binario no encontrado dentro de los hooks del plugin</summary>

El plugin busca el binario primero en `hooks/bin/epic-harness`. Después de actualizar con `cargo install`, cópialo:

```bash
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness
```
</details>

---

## Desarrollo

```bash
cargo install --path .                                        # Compilar + instalar
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness           # Actualizar binario del plugin
cargo test                                                    # Pruebas
```

Los hooks buscan el binario en dos lugares: `hooks/bin/epic-harness` (plugin local) → `~/.cargo/bin/epic-harness` (PATH).

---

## Enlaces

- [Changelog](../../CHANGELOG.md) — historial de versiones
- [Contributing](../../CONTRIBUTING.md) — cómo contribuir
- [Security](../../SECURITY.md) — reportar vulnerabilidades
- [Issues](https://github.com/epicsagas/epic-harness/issues) — informes de errores y solicitudes de funcionalidades

## Agradecimientos

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Patrones de evolución automatizada y benchmarks
- [agent-skills](https://github.com/addyosmani/agent-skills) — Sistema de habilidades de agente de Claude Code
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Patrones exhaustivos de Claude Code
- [gstack](https://github.com/garrytan/gstack) — Referencia de arquitectura de plugins
- [harness](https://github.com/revfactory/harness) — Patrones de infraestructura de hooks y arnés
- [serena](https://github.com/oraios/serena) — Diseño de agentes autónomos
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Arquitectura de framework multi-comando
- [superpowers](https://github.com/obra/superpowers) — Patrones de extensión de Claude Code

## Licencia

[Apache 2.0](../../LICENSE)
