# epic harness

**6 comandos. Skills de activación automática. Auto-evolutivo.**

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

Un plugin de Claude Code que **reemplaza más de 30 comandos con solo 6**, **activa skills automáticamente** según lo que estés haciendo, y **genera nuevos skills** a partir de tus propios patrones de error. Menos superficie que memorizar. Más inteligencia por cada pulsación de tecla.

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness features" width="100%" />
</p>

## Arquitectura: Modelo de 4 Anillos

```
Ring 0 — Piloto automático (hooks, invisible)
  Restauración de sesión, auto-formateo, barreras de seguridad, registro de observaciones

Ring 1 — 6 Comandos (tú los invocas)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — Skills automáticos (activados por contexto)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — Evolución (auto-mejora)
  Observar uso de herramientas → analizar fallos → generar skills automáticamente → validar → recargar
```

## Instalación

```
# Plugin de Claude Code (recomendado)
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# O desde el código fuente
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### Instalar desde binario

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# Desde crates.io
cargo install epic-harness

# Binario pre-compilado (más rápido, sin compilar)
cargo binstall epic-harness

# Desde el código fuente
cargo install --path .
```

El binario se detecta automáticamente por los hooks. Si está ausente, los hooks recurren a Node.js.

## Compatibilidad con múltiples herramientas

epic-harness funciona con Claude Code y 6 herramientas adicionales de programación con IA. Todas las herramientas comparten el mismo directorio de datos `~/.harness/projects/{slug}/`.

| Herramienta | Ring 0 Hooks | Comandos/Prompts | Skills | Agentes |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Completo | ✓ 6 comandos | ✓ 10 skills | ✓ 4 |
| **Codex CLI** | ✓ Completo¹ | ✓ 6 prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Parcial² | ✓ 6 comandos | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Completo³ | ✓ 6 comandos | ✓ via reglas | ✓ 4 |
| **OpenCode** | ✓ Parcial⁴ | ✓ 6 comandos | — | ✓ 4 |
| **Cline** | ✓ Completo⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Requiere `codex_hooks = true` en `~/.codex/config.toml`; PostToolUse intercepta solo Bash
² Sin equivalente `PreToolUse` — guard corre al nivel `BeforeModel`
³ Requiere Cursor 1.7+
⁴ Plugin JS: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ Scripts de hook PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel
⁶ Sin sistema de hooks — convenciones inyectadas via `.aider/CONVENTIONS.md` + `.aider.conf.yml`

### Instalar para otras herramientas

```bash
# Menú interactivo (seleccionar herramientas a instalar)
epic install

# Instalación directa
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (requiere Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Instalación local al proyecto
epic install cursor --local

# Vista previa sin realizar cambios
epic install gemini --dry-run
```

Los archivos de integración en el directorio de la herramienta (`hooks.json`, comandos, agentes, skills, reglas, …) se **sincronizan** desde el binario: los archivos faltantes u obsoletos se escriben. `GEMINI.md` y `AGENTS.md` solo se crean cuando están ausentes.

## Memoria unificada

Todos los agentes comparten un único grafo de conocimiento almacenado en `~/.harness/memory.db` (SQLite + FTS5). No se requiere Node.js ni runtime externo.

### Recuperación inteligente

La recuperación de memoria usa **puntuación compuesta** en vez de simplemente volcar las últimas N entradas:

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **Importancia** configurada automáticamente por tipo de nodo: decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **Seguimiento de acceso**: los recuerdos frecuentemente recuperados suben naturalmente
- **Decaimiento gradual**: los recuerdos sin uso pierden importancia con el tiempo (10% cada 30 días, mínimo 0.05)
- **Aumento del grafo**: la recuperación sigue aristas de 1 salto para traer contexto relacionado

### CLI

```bash
# Recuperación inteligente — clasificada por relevancia para tu tarea actual
epic mem recall "auth refactor" --project my-project

# Añadir un nodo de memoria (importancia auto por tipo, o explícita)
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# Consulta filtrada (incluye importancia + access_count)
epic mem query --type decision --project my-project

# Búsqueda de texto completo (clasificada por importancia)
epic mem search "JWT"

# Contexto inteligente (ponderado por importancia, no solo lo más reciente)
epic mem context --project my-project

# Interfaz Web del grafo de conocimiento
epic mem serve          # → http://localhost:7700

# Registrar como servidor MCP en Claude Code (sin Node.js)
epic mem mcp-install

# Exportar todos los nodos a Markdown para respaldo en Git
epic mem export --out ./docs/memory
```

### Herramientas MCP (6)

Cuando se registra como servidor MCP (`epic mem mcp-install`), los agentes pueden llamar directamente estas herramientas:

| Herramienta | Propósito |
|------|---------|
| `mem_recall` | **Principal.** Recuperación contextual inteligente con hint + proyecto + vecinos del grafo |
| `mem_add` | Añadir nodo con auto-importancia por tipo (o explícita 0.0–1.0) |
| `mem_search` | Búsqueda FTS5, resultados clasificados por importancia |
| `mem_query` | Filtrar por etiqueta/tipo/proyecto |
| `mem_context` | Recuperación inteligente con alcance de proyecto (sin hint) |
| `mem_related` | Recorrido BFS del grafo desde un ID de nodo |

### Cómo funciona el grafo de conocimiento

El grafo se acumula automáticamente a partir del trabajo normal de sesión — no se necesita entrada manual.

**Flujo de datos:**

```
PostToolUse hook → observe (puntuación en 3 ejes) → obs/*.jsonl
                                                          ↓
SessionEnd hook → reflect (detección de patrones) → nodos + aristas memory.db
                                                          ↓  (importancia configurada por tipo)
SessionStart hook → resume (recuperación inteligente) → la próxima sesión recibe hints clasificados por relevancia
                              ↓
                    decay_importance() → los nodos sin uso se desvanecen gradualmente
```

**Tipos de nodos (7):**

| Tipo | Creado por | Importancia por defecto |
|------|-----------|-------------------|
| `decision` | Manual / MCP | 0.9 |
| `resolution` | Manual / MCP | 0.8 |
| `concept` | Manual / MCP | 0.7 |
| `project` | Manual / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**Ciclo de vida de la memoria:**

| Evento | Qué ocurre |
|-------|-------------|
| Nodo recuperado via búsqueda/recall/contexto | `access_count++`, `accessed_at` actualizado |
| 30+ días sin acceso | importancia decae 10% (mínimo 0.05) |
| 180+ días sin acceso | etiquetado `stale`, excluido de la recuperación |
| Nodo etiquetado `pinned` | inmune al decaimiento |

**Condiciones de acumulación automática:**

| Condición | Nodo creado |
|-----------|-------------|
| Cada fin de sesión | `session` (siempre) |
| Mismo error ≥3 veces seguidas | `error` (repeated_same_error) |
| Edit→Error alternando | `pattern` (thrashing) |
| Tasa de éxito de herramienta <60% (mín. 5 observaciones) | `pattern` (weak_tool) |
| Tasa de éxito de tipo de archivo <50% (mín. 3 observaciones) | `pattern` (weak_filetype) |
| Ciclos de éxito en Edit → error en Bash | `pattern` (fix_then_break) |

> **Nota:** Las sesiones limpias (sin errores) solo producen nodos `session`. El grafo se enriquece después de 2–3 sesiones reales de desarrollo con fallos de build, fallos de tests o ciclos de depuración.

Las memorias existentes basadas en archivos (`nodes/*.md`, `edges.jsonl`) se migran automáticamente a SQLite en la primera ejecución.

## Comandos

| Comando | Qué hace |
|---------|----------|
| `/spec` | Define qué construir — clarifica requisitos, produce una especificación |
| `/go` | Constrúyelo — planificación automática, subagentes TDD, ejecución en paralelo |
| `/check` | Verifica — revisión de código + auditoría de seguridad + rendimiento en paralelo |
| `/ship` | Publica — PR, CI, merge |
| `/team` | Crear y sincronizar equipos de agentes a nivel de organización entre proyectos |
| `/evolve` | Activación manual de evolución / estado / rollback |

## Equipos (`epic team`)

Los equipos son de **nivel de organización**, no vinculados a un proyecto. Ejecutar `/team` en cualquier proyecto enriquece un pool compartido de definiciones de agentes — nunca sobrescribe silenciosamente.

### Cómo funciona

```
epic team                      # interactivo: escanear proyecto → diseñar → escribir → sincronizar
         ↓
~/.harness/orgs/epic/teams/backend/   ← almacén global (persiste entre proyectos)
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code descubre automáticamente al iniciar sesión
├── domain-expert.md                  ← definición de rol + contexto de equipo inyectado
├── reviewer.md
└── tester.md
         ↓
Siguiente sesión: agentes activos — seleccionados automáticamente por Claude o llamados explícitamente
```

### Referencia CLI

```bash
# Crear o actualizar un equipo (flujo interactivo de 4 fases)
epic team

# Explorar
epic team list                        # todos los equipos en el org actual
epic team list --org netflix          # equipos en un org con nombre
epic team show backend                # config, misión, agentes
epic team show backend --playbook     # + playbook acumulado completo

# Desplegar a proyecto
epic team sync backend                # desplegar: copiar agentes → .claude/agents/backend/
epic team link backend                # desplegar + registrar proyecto en config del equipo

# Retirar del proyecto
epic team delete backend              # retirar: eliminar solo del proyecto actual
epic team unlink backend              # alias para delete

# Disolver (eliminar completamente del org)
epic team delete backend --global     # eliminar permanentemente del almacén org + copia local

# Historial
epic team history backend reviewer    # listar backups .history/ para un agente
```

### Usar equipos desde agentes de codificación

Tras sincronizar, los agentes están disponibles automáticamente en la siguiente sesión:

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert implementar la pasarela de pagos
@reviewer revisar este PR para casos extremos
@tester escribir tests de integración para auth

# O dejar que el agente seleccione automáticamente según el contexto de la tarea
```

Cada archivo de agente lleva una sección de **Contexto de equipo** inyectada en la sincronización:

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

Los agentes conocen su equipo, misión y cómo cargar el playbook completo bajo demanda —
sin inflar la ventana de contexto con él.

### Multi-org

```bash
epic team                          # acumula en el org "epic" (predeterminado)
epic team --org netflix            # topología Netflix separada
epic team --org client-x           # por cliente
```

Mismo nombre de equipo en el mismo org = compartición intencional entre proyectos.
`epic/teams/backend` acumula conocimiento de cada proyecto que lo crea o vincula.

### Tipos de equipo

| Tipo | Palabra clave | Agentes por defecto |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### Estrategia de fusión — sin sobrescrituras silenciosas

| Objeto | Regla |
|--------|------|
| Agente — nuevo | Añadir automáticamente |
| Agente — sin cambios | Omitir |
| Agente — cambiado | **Solicitar** (predeterminado: mantener existente). Al reemplazar → respaldado en `.history/` |
| `playbook.md` | Siempre **anexar** — nunca truncado |
| `mission.md` — cambiado | **Solicitar** (predeterminado: mantener existente) |

## Skills automáticos (Ring 2)

Los skills se activan automáticamente según el contexto. No necesitas invocarlos.

| Skill | Se activa cuando |
|-------|-----------------|
| **tdd** | Se implementa una nueva funcionalidad |
| **debug** | Fallo en test o error |
| **secure** | Se toca código de autenticación/BD/API/secretos |
| **perf** | Bucles, consultas, código de renderizado |
| **simplify** | Archivo > 200 líneas o alta complejidad |
| **document** | Se añade o modifica API pública |
| **verify** | Antes de completar /go o /ship |
| **context** | Ventana de contexto > 70% utilizada |

## Hooks (Ring 0)

Se ejecutan de forma invisible. No requieren acción del usuario. Implementados como un **único binario Rust** (`epic-harness`) con subcomandos, con retroceso a Node.js si el binario está ausente.

```
epic resume | guard | polish | observe | snapshot | reflect
```

| Hook | Cuándo | Qué hace |
|------|--------|----------|
| **resume** | Inicio de sesión | Restaura contexto, carga memoria, detecta stack |
| **guard** | Antes de Bash | Bloquea force-push a main, rm -rf /, DROP en producción |
| **polish** | Después de Edit | Auto-formateo (Biome/Prettier/ruff/gofmt) + verificación de tipos |
| **observe** | Cada uso de herramienta | Registra en `~/.harness/projects/{slug}/obs/` para la evolución |
| **snapshot** | Antes de compactar | Guarda estado en `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Fin de sesión | Analiza fallos, genera skills evolucionados, valida |

## Sistema de evaluación (Núcleo del Ring 3)

Fusiona los patrones de benchmark de A-Evolve en el sistema de hooks de Claude Code.

### Puntuación multidimensional

Cada llamada a herramienta se puntúa en 3 ejes. Los pesos son configurables mediante `SCORE_WEIGHTS` en `~/.harness/config.toml`:

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (por defecto: 0.5)                      (por defecto: 0.3)                        (por defecto: 0.2)
```

| Dimensión | Qué mide | Criterios por herramienta |
|-----------|----------|--------------------------|
| `tool_success` | ¿Funcionó? (0/1) | Clasificación de fallos en 9 categorías |
| `output_quality` | Señales de calidad de salida (0.0-1.0) | Bash: advertencias, salida vacía. Edit: detección de re-edición |
| `execution_cost` | Indicador de eficiencia (0.0-1.0) | Tamaño de salida, lista blanca de comandos con éxito silencioso |

### Clasificación de fallos (9 categorías)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Detección de patrones (4 tipos)

Todos los umbrales son constantes configurables en `~/.harness/config.toml`:

| Patrón | Detecta | Constante | Por defecto |
|--------|---------|-----------|-------------|
| `repeated_same_error` | Mismo error N+ veces seguidas | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edición exitosa → fallo en build/test | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Atascado en el mismo archivo N+ operaciones | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Alternancia Edit↔Error en el mismo archivo | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Umbrales de generación de skills

| Disparador | Constante | Por defecto |
|------------|-----------|-------------|
| Herramienta débil (baja tasa de éxito) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| Tipo de archivo débil | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| Error de alta frecuencia | `HIGH_FREQ_ERROR_MIN` | 5 |

### Control de estancamiento

- `STAGNATION_LIMIT` (por defecto: 3) sesiones sin mejora → rollback automático de skills evolucionados al mejor checkpoint
- `IMPROVEMENT_THRESHOLD` (por defecto: 5%)
- Seguimiento de tendencia: `improving` / `stable` / `declining` mediante regresión lineal
- Los skills estáticos siempre tienen prioridad sobre los skills evolucionados en caso de conflicto

### Flujo de evolución

```
Observe (PostToolUse — puntuación en 3 ejes)
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: por herramienta, por extensión, distribución de puntuaciones
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 rutas: patrón / herramienta débil / tipo de archivo débil / error frecuente)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate (verificación de formato, deduplicación, límite de 10, verificación de estancamiento)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (mejor checkpoint)
Reload (siguiente sesión — resume.ts reporta métricas + carga skills evolucionados)
```

```bash
/evolve              # Ejecutar evolución ahora
/evolve status       # Panel: puntuaciones, tendencias, patrones, skills
/evolve history      # Análisis a largo plazo: historial completo, efectividad de skills, estadísticas de dispatch
/evolve cross-project # Análisis de patrones entre proyectos
/evolve rollback     # Restaurar el mejor estado anterior
/evolve reset        # Borrar todos los datos de evolución
```

## Presets de arranque en frío

No necesitas esperar 5 sesiones para tener skills evolucionados útiles. En la primera sesión, epic harness detecta tu stack y aplica skills preconfigurados automáticamente:

| Stack | Skills preconfigurados |
|-------|----------------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Los presets son complementarios — se reemplazan por skills evolucionados reales a medida que se acumulan datos.

## Seguridad en sesiones concurrentes

Cada sesión escribe en su propio archivo de observación (`session_{date}_{pid}_{random}.jsonl`). Múltiples sesiones de Claude Code en el mismo proyecto no corromperán los datos entre sí. El hook reflect fusiona todos los archivos del mismo día para el análisis.

## Reglas de protección personalizadas

Añade reglas de seguridad específicas del proyecto mediante `.harness/guard-rules.yaml` en la raíz del proyecto:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Las reglas se combinan con las protecciones integradas (force-push a main, rm -rf /, DROP en producción). Mantener este archivo en git permite compartir reglas de seguridad con tu equipo.

## Aprendizaje entre proyectos

Activa la opción para compartir patrones de fallo entre proyectos:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # activar
```

Cuando está habilitado:
- Al finalizar la sesión, se exportan patrones anonimizados a `~/.harness/global_patterns.jsonl`
- Al iniciar la sesión, se muestran sugerencias de las áreas débiles de otros proyectos
- Usa `/evolve cross-project` para ver patrones agregados

## Seguimiento de efectividad de skills

Cada skill evolucionado se rastrea con puntuaciones de atribución A/B:

```
/evolve history → Sección de efectividad de skills

| Skill              | Sesiones | Puntuación con | Puntuación sin | Delta  |
|--------------------|----------|----------------|----------------|--------|
| evo-ts-care        | 8        | 0.87           | 0.72           | +15%   |
| evo-bash-discipline| 3        | 0.65           | 0.68           | -3%    |
```

Delta positivo = el skill ayuda. Delta negativo = considera eliminarlo con `/evolve rollback`.

## Retroalimentación Polish → Observe

El hook polish (auto-formateo + verificación de tipos) alimenta los resultados de vuelta al pipeline de observación:

- Fallo de formato → registrado como `lint_fail`
- Error de TypeScript → registrado como `build_fail`
- Éxitos → registrados con puntuaciones completas

Esto significa que los patrones de thrashing "editar → error de tipos → editar → error de tipos" se detectan incluso cuando los errores provienen del hook polish, no de comandos manuales.

## Datos del proyecto (`~/.harness/projects/{slug}/`)

Los datos específicos del proyecto residen en tu directorio home. Sobreviven a la eliminación del proyecto y no contaminan el historial git.

```
~/.harness/projects/{slug}/
├── memory/           # Patrones y reglas del proyecto (persistente)
├── sessions/         # Instantáneas de sesión (para restauración)
├── obs/              # Logs de observación de uso de herramientas (JSONL, por sesión)
├── evolved/          # Skills auto-evolucionados
├── evolved_backup/   # Mejor checkpoint (para rollback por estancamiento)
├── dispatch/         # Logs de dispatch de skills (JSONL)
├── team/             # legacy (reemplazado por ~/.harness/orgs/)
├── evolution.jsonl   # Historial completo de evolución
└── metrics.json      # Estadísticas agregadas + atribución de skills

~/.harness/
├── memory.db         # Grafo de conocimiento SQLite (nodos + aristas + FTS5)
├── graph.json        # Grafo en caché (para la interfaz Web)
└── orgs/             # Almacén global epic team
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

Puedes seguir usando `.harness/guard-rules.yaml` en la raíz del proyecto para compartir reglas de seguridad con tu equipo.

## Desarrollo

### Build

```bash
cargo install --path .          # Compilar + instalar en ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Actualizar binario del plugin
```

### Cómo se despachan los hooks

Cada hook en `hooks.json` busca el binario Rust en dos ubicaciones:

```
1. Local del plugin: hooks/bin/epic-harness
2. PATH:             ~/.cargo/bin/epic-harness (vía cargo install)
```

### Tests

```bash
cargo test       # Tests unitarios + de integración de Rust
```

## Agradecimientos

epic harness fue inspirado y construido sobre ideas de los siguientes proyectos:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Evolución automatizada y patrones de benchmark
- [agent-skills](https://github.com/addyosmani/agent-skills) — Sistema de skills para agentes de Claude Code
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Patrones completos de Claude Code
- [gstack](https://github.com/garrytan/gstack) — Referencia de arquitectura de plugins
- [harness](https://github.com/revfactory/harness) — Patrones de infraestructura de hooks y harness
- [serena](https://github.com/oraios/serena) — Diseño de agentes autónomos
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Arquitectura de framework multi-comando
- [superpowers](https://github.com/obra/superpowers) — Patrones de extensión de Claude Code

## Licencia

[Apache 2.0](LICENSE)
