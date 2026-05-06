# epic harness

**6 comandos. Skills de acionamento automático. Auto-evolutivo.**

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

```
# Plugin Claude Code (recomendado)
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# Ou a partir do código-fonte
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### Instalar a partir do binário

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# Via crates.io
cargo install epic-harness

# Binário pré-compilado (mais rápido, sem compilar)
cargo binstall epic-harness

# A partir do código-fonte
cargo install --path .
```

O binário é detectado automaticamente pelos hooks. Se ausente, os hooks recorrem ao Node.js.

## Suporte a Múltiplas Ferramentas

epic-harness funciona com Claude Code e 6 ferramentas adicionais de programação com IA. Todas as ferramentas compartilham o mesmo diretório de dados `~/.harness/projects/{slug}/`.

| Ferramenta | Ring 0 Hooks | Comandos/Prompts | Skills | Agentes |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Completo | ✓ 6 comandos | ✓ 10 skills | ✓ 4 |
| **Codex CLI** | ✓ Completo¹ | ✓ 6 prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Parcial² | ✓ 6 comandos | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Completo³ | ✓ 6 comandos | ✓ via regras | ✓ 4 |
| **OpenCode** | ✓ Parcial⁴ | ✓ 6 comandos | — | ✓ 4 |
| **Cline** | ✓ Completo⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Requer `codex_hooks = true` em `~/.codex/config.toml`; PostToolUse intercepta somente Bash
² Sem equivalente `PreToolUse` — guard corre no nível `BeforeModel`
³ Requer Cursor 1.7+
⁴ Plugin JS: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ Scripts de hook PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel
⁶ Sem sistema de hooks — convenções injetadas via `.aider/CONVENTIONS.md` + `.aider.conf.yml`

### Instalar para outras ferramentas

```bash
# Menu interativo (selecionar ferramentas a instalar)
epic install

# Instalação direta
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (requer Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Instalação local no projeto
epic install cursor --local

# Visualizar sem realizar alterações
epic install gemini --dry-run
```

Os arquivos de integração no diretório da ferramenta (`hooks.json`, comandos, agentes, skills, regras, …) são **sincronizados** a partir do binário: arquivos ausentes ou desatualizados são escritos. `GEMINI.md` e `AGENTS.md` são criados apenas quando ausentes.

## Memória Unificada

Todos os agentes compartilham um único grafo de conhecimento armazenado em `~/.harness/memory.db` (SQLite + FTS5). Nenhum Node.js ou runtime externo necessário.

### Recuperação Inteligente

A recuperação de memória usa **pontuação composta** em vez de simplesmente despejar as últimas N entradas:

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **Importância** configurada automaticamente por tipo de nó: decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **Rastreamento de acesso**: memórias frequentemente recuperadas sobem naturalmente
- **Decaimento gradual**: memórias sem uso perdem importância ao longo do tempo (10% a cada 30 dias, piso 0.05)
- **Aumento do grafo**: a recuperação segue arestas de 1 salto para trazer contexto relacionado

### CLI

```bash
# Recuperação inteligente — classificada por relevância para sua tarefa atual
epic mem recall "auth refactor" --project my-project

# Adicionar um nó de memória (importância auto por tipo, ou explícita)
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# Consulta filtrada (inclui importância + access_count)
epic mem query --type decision --project my-project

# Busca de texto completo (classificada por importância)
epic mem search "JWT"

# Contexto inteligente (ponderado por importância, não apenas o mais recente)
epic mem context --project my-project

# Interface Web do grafo de conhecimento
epic mem serve          # → http://localhost:7700

# Registrar como servidor MCP no Claude Code (sem Node.js)
epic mem mcp-install

# Exportar todos os nós para Markdown para backup no Git
epic mem export --out ./docs/memory
```

### Ferramentas MCP (6)

Quando registrado como servidor MCP (`epic mem mcp-install`), agentes podem chamar diretamente essas ferramentas:

| Ferramenta | Propósito |
|------|---------|
| `mem_recall` | **Principal.** Recuperação contextual inteligente com hint + projeto + vizinhos do grafo |
| `mem_add` | Adicionar nó com auto-importância por tipo (ou explícita 0.0–1.0) |
| `mem_search` | Busca FTS5, resultados classificados por importância |
| `mem_query` | Filtrar por tag/tipo/projeto |
| `mem_context` | Recuperação inteligente com escopo de projeto (sem hint) |
| `mem_related` | Travessia BFS do grafo a partir de um ID de nó |

### Como o Grafo de Conhecimento Funciona

O grafo se acumula automaticamente a partir do trabalho normal de sessão — nenhuma entrada manual necessária.

**Fluxo de dados:**

```
PostToolUse hook → observe (pontuação em 3 eixos) → obs/*.jsonl
                                                          ↓
SessionEnd hook → reflect (detecção de padrões) → nós + arestas memory.db
                                                          ↓  (importância definida por tipo)
SessionStart hook → resume (recuperação inteligente) → próxima sessão recebe hints classificados por relevância
                              ↓
                    decay_importance() → nós sem uso desvanecem gradualmente
```

**Tipos de nós (7):**

| Tipo | Criado por | Importância padrão |
|------|-----------|-------------------|
| `decision` | Manual / MCP | 0.9 |
| `resolution` | Manual / MCP | 0.8 |
| `concept` | Manual / MCP | 0.7 |
| `project` | Manual / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**Ciclo de vida da memória:**

| Evento | O que acontece |
|-------|-------------|
| Nó recuperado via busca/recall/contexto | `access_count++`, `accessed_at` atualizado |
| 30+ dias sem acesso | importância decai 10% (piso 0.05) |
| 180+ dias sem acesso | marcado como `stale`, excluído da recuperação |
| Nó marcado como `pinned` | imune ao decaimento |

**Condições de acumulação automática:**

| Condição | Nó criado |
|-----------|-------------|
| Cada fim de sessão | `session` (sempre) |
| Mesmo erro ≥3 vezes seguidas | `error` (repeated_same_error) |
| Edit→Error alternando | `pattern` (thrashing) |
| Taxa de sucesso da ferramenta <60% (mín. 5 observações) | `pattern` (weak_tool) |
| Taxa de sucesso do tipo de arquivo <50% (mín. 3 observações) | `pattern` (weak_filetype) |
| Ciclos de sucesso em Edit → erro em Bash | `pattern` (fix_then_break) |

> **Nota:** Sessões limpas (sem erros) produzem apenas nós `session`. O grafo se enriquece após 2–3 sessões reais de desenvolvimento com falhas de build, falhas de testes ou ciclos de depuração.

Memórias existentes baseadas em arquivos (`nodes/*.md`, `edges.jsonl`) são automaticamente migradas para SQLite na primeira execução.

## Comandos

| Comando | O que faz |
|---------|-----------|
| `/spec` | Define o que construir — esclarece requisitos, produz uma especificação |
| `/go` | Constrói — planejamento automático, subagentes TDD, execução paralela |
| `/check` | Verifica — revisão de código + auditoria de segurança + performance em paralelo |
| `/ship` | Entrega — PR, CI, merge |
| `/team` | Criar e sincronizar equipes de agentes em nível de organização entre projetos |
| `/evolve` | Acionamento manual de evolução / status / rollback |

## Equipes (`epic team`)

As equipes são de **nível de organização**, não vinculadas a um projeto. Executar `/team` em qualquer projeto enriquece um pool compartilhado de definições de agentes — nunca sobrescreve silenciosamente.

### Como funciona

```
epic team                      # interativo: escanear projeto → projetar → escrever → sincronizar
         ↓
~/.harness/orgs/epic/teams/backend/   ← armazenamento global (persiste entre projetos)
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code descobre automaticamente ao iniciar sessão
├── domain-expert.md                  ← definição de papel + contexto de equipe injetado
├── reviewer.md
└── tester.md
         ↓
Próxima sessão: agentes ativos — selecionados automaticamente por Claude ou chamados explicitamente
```

### Referência CLI

```bash
# Criar ou atualizar uma equipe (fluxo interativo de 4 fases)
epic team

# Explorar
epic team list                        # todas as equipes na org atual
epic team list --org netflix          # equipes em uma org com nome
epic team show backend                # config, missão, agentes
epic team show backend --playbook     # + playbook acumulado completo

# Implantar no projeto
epic team sync backend                # implantar: copiar agentes → .claude/agents/backend/
epic team link backend                # implantar + registrar projeto na config da equipe

# Retirar do projeto
epic team delete backend              # retirar: remover apenas do projeto atual
epic team unlink backend              # alias para delete

# Dissolver (remover completamente da org)
epic team delete backend --global     # excluir permanentemente do armazenamento da org + cópia local

# Histórico
epic team history backend reviewer    # listar backups .history/ para um agente
```

### Usar equipes a partir de agentes de codificação

Após sincronizar, os agentes estão disponíveis automaticamente na próxima sessão:

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert implementar o gateway de pagamento
@reviewer verificar este PR para casos extremos
@tester escrever testes de integração para auth

# Ou deixar o agente selecionar automaticamente com base no contexto da tarefa
```

Cada arquivo de agente carrega uma seção de **Contexto de equipe** injetada na sincronização:

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

Os agentes conhecem sua equipe, missão e como carregar o playbook completo sob demanda —
sem inflar a janela de contexto com ele.

### Multi-org

```bash
epic team                          # acumula na org "epic" (padrão)
epic team --org netflix            # topologia Netflix separada
epic team --org client-x           # por cliente
```

Mesmo nome de equipe na mesma org = compartilhamento intencional entre projetos.
`epic/teams/backend` acumula conhecimento de cada projeto que o cria ou vincula.

### Tipos de equipe

| Tipo | Palavra-chave | Agentes padrão |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### Estratégia de mesclagem — sem sobrescritas silenciosas

| Objeto | Regra |
|--------|------|
| Agente — novo | Adicionar automaticamente |
| Agente — sem alterações | Pular |
| Agente — alterado | **Solicitar** (padrão: manter existente). Ao substituir → salvo em `.history/` |
| `playbook.md` | Sempre **anexar** — nunca truncado |
| `mission.md` — alterado | **Solicitar** (padrão: manter existente) |

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

Executam de forma invisível. Nenhuma ação do usuário é necessária. Implementados como um **único binário Rust** (`epic-harness`) com subcomandos, com fallback para Node.js se o binário estiver ausente.

```
epic resume | guard | polish | observe | snapshot | reflect
```

| Hook | Quando | O que faz |
|------|--------|-----------|
| **resume** | Início da sessão | Restaura contexto, carrega memória, detecta stack |
| **guard** | Antes do Bash | Bloqueia force-push-to-main, rm -rf /, DROP prod |
| **polish** | Após Edit | Auto-formatação (Biome/Prettier/ruff/gofmt) + verificação de tipos |
| **observe** | A cada uso de ferramenta | Registra em `~/.harness/projects/{slug}/obs/` para evolução |
| **snapshot** | Antes de compactar | Salva estado em `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Fim da sessão | Analisa falhas, semeia skills evoluídas, validação |

## Sistema de Avaliação (Núcleo do Ring 3)

Integra os padrões de benchmark do A-Evolve ao sistema de hooks do Claude Code.

### Pontuação Multidimensional

Cada chamada de ferramenta é avaliada em 3 eixos. Os pesos são configuráveis via `SCORE_WEIGHTS` em `~/.harness/config.toml`:

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

Todos os limites são constantes configuráveis em `~/.harness/config.toml`:

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
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: por ferramenta, por extensão, distribuição de pontuação
    ↓ Padrões: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 caminhos: padrão / ferramenta fraca / tipo de arquivo fraco / erro frequente)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate (verificação de formato, dedup, limite de 10, verificação de estagnação)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (melhor checkpoint)
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

Adicione regras de segurança específicas do projeto via `.harness/guard-rules.yaml` na raiz do projeto:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

As regras são combinadas com as proteções integradas (force-push-to-main, rm -rf /, DROP prod). Manter este arquivo no git permite compartilhar regras de segurança com sua equipe.

## Aprendizado Entre Projetos

Opte por compartilhar padrões de falha entre projetos:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # ativar
```

Quando ativado:
- No fim da sessão, padrões anonimizados são exportados para `~/.harness/global_patterns.jsonl`
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

## Dados do Projeto (`~/.harness/projects/{slug}/`)

Dados específicos do projeto ficam no seu diretório home. Sobrevivem à exclusão do projeto e não poluem o histórico git.

```
~/.harness/projects/{slug}/
├── memory/           # Padrões e regras do projeto (persistente)
├── sessions/         # Snapshots de sessão (para restauração)
├── obs/              # Logs de observação de uso de ferramentas (JSONL, por sessão)
├── evolved/          # Skills auto-evoluídas
├── evolved_backup/   # Melhor checkpoint (para rollback de estagnação)
├── dispatch/         # Logs de dispatch de skills (JSONL)
├── team/             # legacy (substituído por ~/.harness/orgs/)
├── evolution.jsonl   # Histórico completo de evolução
└── metrics.json      # Estatísticas agregadas + atribuição de skills

~/.harness/
├── memory.db         # Grafo de conhecimento SQLite (nós + arestas + FTS5)
├── graph.json        # Grafo em cache (para a interface Web)
└── orgs/             # Armazenamento global epic team
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

Você ainda pode usar `.harness/guard-rules.yaml` na raiz do projeto para compartilhar regras de segurança com sua equipe.

## Desenvolvimento

### Build

```bash
cargo install --path .          # Compilar + instalar em ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Atualizar binário do plugin
```

### Como os hooks são despachados

Cada hook em `hooks.json` procura o binário Rust em dois locais:

```
1. Local do plugin: hooks/bin/epic-harness
2. PATH:            ~/.cargo/bin/epic-harness (via cargo install)
```

### Testes

```bash
cargo test       # Testes unitários + de integração Rust
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
