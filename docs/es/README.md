# epic harness

**6 comandos. Skills de activación automática. Auto-evolutivo.**

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

```bash
# Marketplace de plugins de Claude Code
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# O manualmente
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Binario Rust (opcional, ~4x más rápido en hooks)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# Desde crates.io
cargo install epic-harness
# o con cargo-binstall (pre-compilado, más rápido)
cargo binstall epic-harness

# Desde el código fuente
cargo install --path .
```

El binario se detecta automáticamente por los hooks. Si no está presente, los hooks recurren a Node.js como respaldo.

## Compatibilidad con múltiples herramientas

epic-harness funciona con Claude Code y 6 herramientas adicionales de programación con IA. Todas las herramientas comparten el mismo directorio de datos `~/.harness/projects/{slug}/`.

| Tool | Ring 0 Hooks | Commands/Prompts | Skills | Agents |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Full | ✓ 6 commands | ✓ 8 skills | ✓ 4 |
| **Codex CLI** | ✓ Full¹ | ✓ 6 prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Partial² | ✓ 6 commands | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Full³ | ✓ 6 commands | ✓ via rules | ✓ 4 |
| **OpenCode** | ✓ Partial⁴ | ✓ 6 commands | — | ✓ 4 |
| **Cline** | ✓ Full⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Requires `codex_hooks = true` in `~/.codex/config.toml`; PostToolUse intercepts Bash only  
² No `PreToolUse` equivalent — guard runs at `BeforeModel` level  
³ Requires Cursor 1.7+  
⁴ JS plugin: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`  
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel hook scripts  
⁶ No hook system — conventions injected via `.aider/CONVENTIONS.md` + `.aider.conf.yml`

### Instalar para otras herramientas

```bash
# Menú interactivo
epic-harness install

# Instalación directa
epic-harness install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic-harness install gemini       # Gemini CLI  → ~/.gemini/
epic-harness install cursor       # Cursor      → ~/.cursor/ (requires Cursor 1.7+)
epic-harness install opencode     # OpenCode    → ~/.config/opencode/
epic-harness install cline        # Cline       → ~/Documents/Cline/Rules/
epic-harness install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Instalación local al proyecto
epic-harness install cursor --local

# Vista previa sin realizar cambios
epic-harness install gemini --dry-run
```

## Comandos

| Comando | Qué hace |
|---------|----------|
| `/spec` | Define qué construir — clarifica requisitos, produce una especificación |
| `/go` | Constrúyelo — planificación automática, subagentes TDD, ejecución en paralelo |
| `/check` | Verifica — revisión de código + auditoría de seguridad + rendimiento en paralelo |
| `/ship` | Publica — PR, CI, merge |
| `/team` | Diseña un equipo de agentes específico para el proyecto |
| `/evolve` | Activación manual de evolución / estado / rollback |

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

Se ejecutan de forma invisible. No requieren acción del usuario. Implementados como un **único binario Rust** (`epic-harness`) con subcomandos, con respaldo en Node.js si el binario no está disponible.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| Hook | Cuándo | Qué hace |
|------|--------|----------|
| **resume** | Inicio de sesión | Restaura contexto, carga memoria, detecta stack |
| **guard** | Antes de Bash | Bloquea force-push a main, rm -rf /, DROP en producción |
| **polish** | Después de Edit | Auto-formateo (Biome/Prettier/ruff/gofmt) + verificación de tipos |
| **observe** | Cada uso de herramienta | Registra en `.harness/obs/` para la evolución |
| **snapshot** | Antes de compactar | Guarda estado en `.harness/sessions/` |
| **reflect** | Fin de sesión | Analiza fallos, genera skills evolucionados, valida |

## Sistema de evaluación (Núcleo del Ring 3)

Fusiona los patrones de benchmark de A-Evolve en el sistema de hooks de Claude Code.

### Puntuación multidimensional

Cada llamada a herramienta se puntúa en 3 ejes. Los pesos son configurables mediante `SCORE_WEIGHTS` en `src/ts/common.ts` (o `src/hooks/common.rs`):

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

Todos los umbrales son constantes configurables en `src/ts/common.ts` (o `src/hooks/common.rs`):

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
    ↓ .harness/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: por herramienta, por extensión, distribución de puntuaciones
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 rutas: patrón / herramienta débil / tipo de archivo débil / error frecuente)
    ↓ .harness/evolved/{skill}/SKILL.md
Gate (verificación de formato, deduplicación, límite de 10, verificación de estancamiento)
    ↓ .harness/evolved_backup/ (mejor checkpoint)
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

Añade reglas de seguridad específicas del proyecto mediante `.harness/guard-rules.yaml`:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Las reglas se combinan con las protecciones integradas (force-push a main, rm -rf /, DROP en producción).

## Aprendizaje entre proyectos

Activa la opción para compartir patrones de fallo entre proyectos:

```bash
touch .harness/.cross-project-enabled  # activar
```

Cuando está habilitado:
- Al finalizar la sesión, se exportan patrones anonimizados a `~/.harness-global/patterns.jsonl`
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

## Datos del proyecto (`.harness/`)

epic harness crea un directorio `.harness/` en tu proyecto:

```
.harness/
├── memory/           # Patrones y reglas del proyecto (persistente)
├── sessions/         # Instantáneas de sesión (para restauración)
├── obs/              # Logs de observación de uso de herramientas (JSONL, por sesión)
├── evolved/          # Skills auto-evolucionados
├── evolved_backup/   # Mejor checkpoint (para rollback por estancamiento)
├── dispatch/         # Logs de dispatch de skills (JSONL)
├── team/             # Agentes y skills generados por /team
├── evolution.jsonl   # Historial completo de evolución
├── metrics.json      # Estadísticas agregadas + atribución de skills
└── guard-rules.yaml  # Reglas de protección personalizadas (opcional)
```

Añade `.harness/` a `.gitignore` o haz commit — tú decides.

## Desarrollo

### Rust (principal — ~4x más rápido)

```bash
cargo install --path .          # Compilar + instalar en ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Actualizar binario del plugin
```

### Node.js (respaldo)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### Cómo se despachan los hooks

Cada hook en `hooks.json` busca el binario Rust en tres ubicaciones, y luego recurre a Node.js como respaldo:

```
1. Local del plugin: hooks/bin/epic-harness
2. PATH:             ~/.cargo/bin/epic-harness (vía cargo install)
3. Respaldo:         node hooks/scripts/<hook>.js
```

### Tests

```bash
cargo test       # 98 tests unitarios de Rust
npm test         # Tests unitarios + e2e de Node.js
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
