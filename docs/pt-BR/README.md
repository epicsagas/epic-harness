# epic harness

**6 comandos. Skills de acionamento automático. Auto-evolutivo.**

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

Um plugin para Claude Code que **substitui mais de 30 comandos por apenas 6**, **aciona skills automaticamente** com base no que você está fazendo e **evolui novas skills** a partir dos seus próprios padrões de falha. Menos superfície para memorizar. Mais inteligência por tecla pressionada.

<p align="center">
  <img src="../../assets/features.jpg" alt="funcionalidades do epic harness" width="100%" />
</p>

## Arquitetura: Modelo de 4 Anéis

```
Ring 0 — Autopilot (hooks, invisível)
  Restauração de sessão, auto-formatação, barreiras de segurança, registro de observações

Ring 1 — 6 Comandos (você os invoca)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — Auto Skills (acionadas por contexto)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — Evolve (auto-aprimoramento)
  Observa uso de ferramentas → analisa falhas → gera skills automaticamente → validação → recarga
```

## Instalação

```bash
# Marketplace de plugins do Claude Code
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# Ou manualmente
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Binário Rust (opcional, ~4x mais rápido nos hooks)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# Via crates.io
cargo install epic-harness
# ou com cargo-binstall (pré-compilado, mais rápido)
cargo binstall epic-harness

# A partir do código-fonte
cargo install --path .
```

O binário é detectado automaticamente pelos hooks. Se ausente, os hooks utilizam Node.js como fallback.

## Suporte a Múltiplas Ferramentas

epic-harness funciona com Claude Code e 6 ferramentas adicionais de programação com IA. Todas as ferramentas compartilham o mesmo diretório de dados `~/.harness/projects/{slug}/`.

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

### Instalar para outras ferramentas

```bash
# Menu interativo
epic-harness install

# Instalação direta
epic-harness install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic-harness install gemini       # Gemini CLI  → ~/.gemini/
epic-harness install cursor       # Cursor      → ~/.cursor/ (requires Cursor 1.7+)
epic-harness install opencode     # OpenCode    → ~/.config/opencode/
epic-harness install cline        # Cline       → ~/Documents/Cline/Rules/
epic-harness install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Instalação local no projeto
epic-harness install cursor --local

# Visualizar sem realizar alterações
epic-harness install gemini --dry-run
```

## Memória Unificada

Todos os agentes compartilham um único grafo de conhecimento em `~/.harness/memory/`.

```bash
# Adicionar uma decisão
harness mem add "auth usa cookies de sessão em vez de JWT"

# Busca semântica
harness mem query "abordagem de autenticação"

# Busca por texto completo
harness mem search "JWT"

# Iniciar a interface Web do grafo de conhecimento D3.js (http://localhost:7700)
harness mem serve

# Registrar servidor MCP para Claude Code (5 ferramentas nativas: mem_add, mem_query, mem_search, mem_related, mem_context)
harness mem mcp-install

# Migrar memórias existentes por projeto
harness mem migrate --all
```

Os agentes registram decisões de arquitetura automaticamente via hooks PostToolUse. No início da sessão, memórias relevantes são injetadas no contexto.

## Comandos

| Comando | O que faz |
|---------|-----------|
| `/spec` | Define o que construir — esclarece requisitos, produz uma especificação |
| `/go` | Constrói — planejamento automático, subagentes TDD, execução paralela |
| `/check` | Verifica — revisão de código + auditoria de segurança + performance em paralelo |
| `/ship` | Entrega — PR, CI, merge |
| `/team` | Projeta uma equipe de agentes específica para o projeto |
| `/evolve` | Acionamento manual de evolução / status / rollback |

## Auto Skills (Ring 2)

As skills são acionadas automaticamente com base no contexto. Você não precisa invocá-las.

| Skill | Aciona quando |
|-------|---------------|
| **tdd** | Implementação de nova funcionalidade |
| **debug** | Falha em teste ou erro |
| **secure** | Código de autenticação/BD/API/secrets é alterado |
| **perf** | Loops, queries, código de renderização |
| **simplify** | Arquivo com mais de 200 linhas ou alta complexidade |
| **document** | API pública adicionada ou alterada |
| **verify** | Antes de completar /go ou /ship |
| **context** | Janela de contexto > 70% utilizada |

## Hooks (Ring 0)

Executam de forma invisível. Nenhuma ação do usuário é necessária. Implementados como um **único binário Rust** (`epic-harness`) com subcomandos, com fallback para Node.js caso o binário não esteja disponível.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| Hook | Quando | O que faz |
|------|--------|-----------|
| **resume** | Início da sessão | Restaura contexto, carrega memória, detecta stack |
| **guard** | Antes do Bash | Bloqueia force-push-to-main, rm -rf /, DROP prod |
| **polish** | Após Edit | Auto-formatação (Biome/Prettier/ruff/gofmt) + verificação de tipos |
| **observe** | A cada uso de ferramenta | Registra em `.harness/obs/` para evolução |
| **snapshot** | Antes de compactar | Salva estado em `.harness/sessions/` |
| **reflect** | Fim da sessão | Analisa falhas, semeia skills evoluídas, validação |

## Sistema de Avaliação (Núcleo do Ring 3)

Integra os padrões de benchmark do A-Evolve ao sistema de hooks do Claude Code.

### Pontuação Multidimensional

Cada chamada de ferramenta é avaliada em 3 eixos. Os pesos são configuráveis via `SCORE_WEIGHTS` em `src/ts/common.ts` (ou `src/hooks/common.rs`):

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (padrão: 0.5)                          (padrão: 0.3)                             (padrão: 0.2)
```

| Dimensão | O que mede | Critérios por ferramenta |
|----------|-----------|--------------------------|
| `tool_success` | Funcionou? (0/1) | Classificação de falhas em 9 categorias |
| `output_quality` | Sinais de qualidade da saída (0.0-1.0) | Bash: avisos, saída vazia. Edit: detecção de reedição |
| `execution_cost` | Proxy de eficiência (0.0-1.0) | Tamanho da saída, whitelist de comandos silenciosos |

### Classificação de Falhas (9 categorias)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Detecção de Padrões (4 tipos)

Todos os limites são constantes configuráveis em `src/ts/common.ts` (ou `src/hooks/common.rs`):

| Padrão | Detecta | Constante | Padrão |
|--------|---------|-----------|--------|
| `repeated_same_error` | Mesmo erro N+ vezes consecutivas | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edição bem-sucedida → build/teste falha | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Preso no mesmo arquivo por N+ operações | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Alternância Edição↔Erro no mesmo arquivo | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Limites de Semeadura de Skills

| Gatilho | Constante | Padrão |
|---------|-----------|--------|
| Ferramenta fraca (baixa taxa de sucesso) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| Tipo de arquivo fraco | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| Erro de alta frequência | `HIGH_FREQ_ERROR_MIN` | 5 |

### Controle de Estagnação

- `STAGNATION_LIMIT` (padrão: 3) sessões sem melhoria → rollback automático das skills evoluídas para o melhor checkpoint
- `IMPROVEMENT_THRESHOLD` (padrão: 5%)
- Rastreamento de tendência: `improving` / `stable` / `declining` via regressão linear
- Skills estáticas sempre têm prioridade sobre skills evoluídas em caso de conflito

### Fluxo de Evolução

```
Observe (PostToolUse — pontuação em 3 eixos)
    ↓ .harness/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: por ferramenta, por extensão, distribuição de pontuação
    ↓ Padrões: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 caminhos: padrão / ferramenta fraca / tipo de arquivo fraco / erro frequente)
    ↓ .harness/evolved/{skill}/SKILL.md
Gate (verificação de formato, dedup, limite de 10, verificação de estagnação)
    ↓ .harness/evolved_backup/ (melhor checkpoint)
Reload (próxima sessão — resume.ts reporta métricas + carrega skills evoluídas)
```

```bash
/evolve              # Executar evolução agora
/evolve status       # Painel: pontuações, tendências, padrões, skills
/evolve history      # Análise de longo prazo: histórico completo, eficácia das skills, estatísticas de dispatch
/evolve cross-project # Análise de padrões entre projetos
/evolve rollback     # Restaurar melhor estado anterior
/evolve reset        # Limpar todos os dados de evolução
```

## Presets de Início Rápido

Não é necessário esperar 5 sessões para obter skills evoluídas úteis. Na primeira sessão, o epic harness detecta sua stack e aplica skills predefinidas automaticamente:

| Stack | Skills Predefinidas |
|-------|---------------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Os presets são suplementares — são substituídos por skills realmente evoluídas conforme os dados se acumulam.

## Segurança em Sessões Concorrentes

Cada sessão grava em seu próprio arquivo de observação (`session_{date}_{pid}_{random}.jsonl`). Múltiplas sessões do Claude Code no mesmo projeto não corrompem os dados umas das outras. O hook reflect mescla todos os arquivos do mesmo dia para análise.

## Regras de Proteção Personalizadas

Adicione regras de segurança específicas do projeto via `.harness/guard-rules.yaml`:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

As regras são combinadas com as proteções integradas (force-push-to-main, rm -rf /, DROP prod).

## Aprendizado Entre Projetos

Opte por compartilhar padrões de falha entre projetos:

```bash
touch .harness/.cross-project-enabled  # ativar
```

Quando ativado:
- No fim da sessão, padrões anonimizados são exportados para `~/.harness-global/patterns.jsonl`
- No início da sessão, dicas de áreas fracas de outros projetos são exibidas
- Use `/evolve cross-project` para ver padrões agregados

## Rastreamento de Eficácia das Skills

Cada skill evoluída é rastreada com pontuações de atribuição A/B:

```
/evolve history → Seção de Eficácia das Skills

| Skill              | Sessões  | Pontuação Com | Pontuação Sem | Delta  |
|--------------------|----------|---------------|---------------|--------|
| evo-ts-care        | 8        | 0.87          | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65          | 0.68          | -3%    |
```

Delta positivo = a skill ajuda. Delta negativo = considere remover via `/evolve rollback`.

## Feedback Polish → Observe

O hook polish (auto-formatação + verificação de tipos) alimenta os resultados de volta no pipeline de observação:

- Falha de formatação → registrada como `lint_fail`
- Erro de TypeScript → registrado como `build_fail`
- Sucessos → registrados com pontuações completas

Isso significa que padrões de thrashing "editar → erro de tipo → editar → erro de tipo" são detectados mesmo quando os erros vêm do hook polish, não de comandos manuais.

## Dados do Projeto (`.harness/`)

O epic harness cria um diretório `.harness/` no seu projeto:

```
.harness/
├── memory/           # Padrões e regras do projeto (persistente)
├── sessions/         # Snapshots de sessão (para restauração)
├── obs/              # Logs de observação de uso de ferramentas (JSONL, por sessão)
├── evolved/          # Skills auto-evoluídas
├── evolved_backup/   # Melhor checkpoint (para rollback de estagnação)
├── dispatch/         # Logs de dispatch de skills (JSONL)
├── team/             # Agentes e skills gerados pelo /team
├── evolution.jsonl   # Histórico completo de evolução
├── metrics.json      # Estatísticas agregadas + atribuição de skills
└── guard-rules.yaml  # Regras de proteção personalizadas (opcional)
```

Adicione `.harness/` ao `.gitignore` ou faça commit — a escolha é sua.

## Desenvolvimento

### Rust (primário — ~4x mais rápido)

```bash
cargo install --path .          # Compilar + instalar em ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Atualizar binário do plugin
```

### Node.js (fallback)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### Como os hooks são despachados

Cada hook em `hooks.json` procura o binário Rust em três locais, depois faz fallback para Node.js:

```
1. Local do plugin: hooks/bin/epic-harness
2. PATH:            ~/.cargo/bin/epic-harness (via cargo install)
3. Fallback:        node hooks/scripts/<hook>.js
```

### Testes

```bash
cargo test       # 98 testes unitários Rust
npm test         # Testes unitários + e2e Node.js
```

## Agradecimentos

O epic harness foi inspirado e construído com base em ideias dos seguintes projetos:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Evolução automatizada e padrões de benchmark
- [agent-skills](https://github.com/addyosmani/agent-skills) — Sistema de skills para agentes do Claude Code
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Padrões abrangentes para Claude Code
- [gstack](https://github.com/garrytan/gstack) — Referência de arquitetura de plugins
- [harness](https://github.com/revfactory/harness) — Padrões de infraestrutura de hooks e harness
- [serena](https://github.com/oraios/serena) — Design de agentes autônomos
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Arquitetura de framework multi-comando
- [superpowers](https://github.com/obra/superpowers) — Padrões de extensão do Claude Code

## Licença

[Apache 2.0](LICENSE)
