<h1 align="center">Epic Harness</h1>

<blockquote><p align="center">Um harness de agente de codificação IA autoevolutivo — 8 comandos, 1 pipeline autônomo, habilidades de ativação automática, aprende com seus erros.</p></blockquote>

<p align="center"><b>Menos para memorizar. Mais inteligência por tecla pressionada. Fica mais inteligente a cada sessão.</b></p>

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
  <img alt="Version" src="https://img.shields.io/badge/version-0.3.11-fc8d62?style=for-the-badge&labelColor=0d1117" />
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.82+-d73a49?style=for-the-badge&labelColor=0d1117&logo=rust&logoColor=white" />
  <img alt="Claude Code" src="https://img.shields.io/badge/Claude_Code-plugin-bc8cff?style=for-the-badge&labelColor=0d1117" />
  <a href="https://buymeacoffee.com/epicsaga"><img alt="Buy Me a Coffee" src="https://img.shields.io/badge/buy_me_a_coffee-FFDD00?style=for-the-badge&labelColor=0d1117&logo=buymeacoffee&logoColor=black" /></a>
</p>

Um plugin do Claude Code que **substitui mais de 30 comandos por 8**, **ativa habilidades automaticamente** com base no que você está fazendo, e **evolui novas habilidades** a partir dos seus próprios padrões de falha.

<p align="center">
  <img src="../../assets/features.png" alt="funcionalidades do epic harness" width="100%" />
</p>

---

![Demo](../../docs/demo/demo.gif)

### Painel web — 10 telas com métricas em tempo real
<p align="center">
  <img src="../../assets/dashboard.png" alt="Dashboard" width="49%" />
  <img src="../../assets/dashboard-orbit.png" alt="Orbit Pipeline" width="49%" />
</p>

---

## O Que Ele Faz

Um comando entrega uma funcionalidade de ponta a ponta. As habilidades são ativadas sem você pedir. O agente fica mais inteligente a cada sessão.

```bash
$ /orbit "Adicionar autenticação JWT à API de login"
→ spec approved → go (TDD subagents) → check (PASS) → ship (PR + CI) → evolve
```

Ou avance passo a passo manualmente:

```bash
/spec "Adicionar autenticação JWT à API de login"   # clarifica requisitos → SPEC-*.md
/go                                                   # planejamento automático → subagentes TDD → 4 min
/check                                                # revisão paralela + segurança + testes → PASS
/ship                                                 # teste isolado → PR → CI verde
```

As habilidades são ativadas automaticamente em segundo plano — sem comandos extras:

```
Construindo uma feature?        → tdd dispara (Red→Green→Refactor obrigatório)
Teste falhou?                   → debug dispara (causa raiz primeiro, sem correções aleatórias)
Mexeu em auth ou DB?            → secure dispara (checklist OWASP, sem atalhos)
Arquivo passou de 200 linhas?   → simplify dispara (extrair, renomear, reduzir)
```

Após o encerramento da sessão, o **loop evolve** analisa o que quebrou, gera habilidades direcionadas e as carrega na próxima sessão. O agente que teve dificuldades com falhas de build do TypeScript terá uma habilidade `evo-ts-care` na próxima vez.

---

## Instalação

> **Primeira vez?** Leia o [Guia de Início Rápido (5 min)](../../docs/quickstart.md).

### Claude Code (recomendado)

```
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

Instala automaticamente o binário e registra todos os hooks em uma única etapa.

### macOS / Linux

```bash
brew install epicsagas/tap/epic-harness
```

Sem Homebrew? Use o script de instalação:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/epic-harness/releases/latest/download/epic-harness-installer.ps1 | iex
```

### Via toolchain Rust

```bash
cargo binstall epic-harness   # binário pré-compilado (rápido)
cargo install epic-harness    # compilar a partir do código-fonte
```

Depois execute o assistente de configuração:

```bash
epic install          # Claude Code (padrão)
epic install codex    # Codex CLI
epic install gemini   # Gemini CLI
```

> Use `epic-harness --version` para verificar. Atualize com `brew upgrade epic-harness` ou execute o script de instalação novamente.

Pré-requisitos: **Git**. Instalações via código-fonte/binário também precisam da [toolchain Rust](https://rustup.rs).

### `epic install` — assistente de configuração

Após instalar o binário, execute `epic install` (ou `epic install claude`) para:

1. Criar a estrutura de diretórios `~/.harness/`
2. Sincronizar comandos, habilidades e agentes no diretório de configuração da ferramenta
3. Registrar o servidor MCP (harness-mem) para o Claude Code
4. Criar `~/.harness/config.toml` com valores padrão, se ausente

No Claude Code, `hooks/setup.sh` é executado automaticamente no início da sessão e instala o binário se estiver ausente. Nenhuma etapa manual é necessária após o clone inicial.

### Outras ferramentas

```bash
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (requer Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/
epic install              # Menu interativo
```

Os arquivos de integração são **sincronizados** a partir do binário: arquivos ausentes ou desatualizados são escritos. `GEMINI.md` e `AGENTS.md` são criados apenas quando ausentes.

### Verificação

```bash
epic --version              # Binário instalado
ls ~/.harness/              # Diretório de dados existe
```

Dentro de uma sessão do Claude Code: `/evolve status`

---

## Comandos

| Comando | O que faz |
|---------|-----------|
| `/orbit` | **Pipeline autônomo completo**: spec → go → check → ship → evolve em uma única execução |
| `/discover` | Enquadre o problema primeiro — 5 Porquês, JTBD, questionamento socrático (máx. 3 rodadas) |
| `/spec` | Converte requisitos em um documento numerado de R + AC, salvo como `SPEC-{timestamp}.md` |
| `/go` | Planejamento automático → subagentes TDD → execução paralela com isolamento de worktree → verificação de AC |
| `/check` | Revisão paralela + auditoria de segurança + testes, com extras baseados em escopo (contrato de API, acessibilidade, segurança de migração) |
| `/ship` | Teste de pré-voo isolado em worktree limpa → PR com relatório de verificação completo → monitoramento de CI + correção automática |
| `/team` | Navegue pelas bibliotecas da organização, contrate equipes existentes ou crie novas (3–6 agentes, sincronizados em `.claude/agents/`) |
| `/evolve` | Gatilho manual de evolução — analise sessões, visualize o painel, inspecione a efetividade das habilidades, rollback |

---

## /orbit — Pipeline Autônomo

`/orbit` encapsula todo o pipeline em uma única execução autônoma. Escolha um modo — todo o resto é automático até o PR.

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

**Roxo** — etapas humanas: seleção de modo (unclear → interativo), pausa após 3 falhas de check.
**Verde** — clear + complex → auto-spec em conselho; clear + simple → build direto; ambos totalmente autônomos.

Estado persistido em `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` — sobrevive à compactação de contexto.

> **Ressalvas**: O agente pode ignorar o pipeline ao modificar o próprio orbit ou editar apenas documentação. Veja [Problemas Conhecidos (Julgamento do Agente)](#problemas-conhecidos-julgamento-do-agente).

---

## Habilidades Automáticas (Ring 2)

As habilidades são ativadas automaticamente com base no contexto. Você não as invoca.

| Habilidade | Ativa quando |
|------------|-------------|
| **tdd** | Implementação de nova funcionalidade ou correção de bug |
| **debug** | Falha de teste ou erro em tempo de execução |
| **discover** | Solicitação vaga, solução sem problema, reclamação sem foco |
| **secure** | Código de Auth / DB / API / secrets modificado |
| **perf** | Loops, consultas, renderização, operações em lote |
| **simplify** | Arquivo com mais de 200 linhas ou alta complexidade ciclomática |
| **document** | API pública adicionada ou assinatura alterada |
| **verify** | Antes de completar `/go` ou `/ship` |
| **context** | Janela de contexto > 70% |
| **council** | Decisões arquiteturais ou de design ambíguas |
| **agent-introspection** | 3+ falhas consecutivas ou padrão de retry circular |
| **reflect** | Sob demanda: você está usando a IA como amplificador de pensamento? Autoavaliação baseada em evidências |

---

## Evolve (Ring 3)

O harness monitora cada chamada de ferramenta, pontua em 3 eixos, detecta padrões de falha e gera habilidades direcionadas — automaticamente, ao final da sessão.

### Pontuação

```
composite = 0.5 × tool_success + 0.3 × output_quality + 0.2 × execution_cost
```

Classificação de falhas (9 tipos): `type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Detecção de Padrões

| Padrão | Detecta | Limiar padrão |
|--------|---------|---------------|
| `repeated_same_error` | Mesmo erro N+ vezes | 3 |
| `fix_then_break` | Sucesso na edição → falha de build/teste | 3 lookback, 2 ciclos |
| `long_debug_loop` | Preso no mesmo arquivo | 5 operações |
| `thrashing` | Alternância Edit↔Error | 3 edições, 3 erros |

### Fluxo de Evolução

```
Observe (PostToolUse — pontuação em 3 eixos)
    ↓ obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ pontuações por ferramenta, por extensão + padrões
Propose (Solver — graduado por pontuação: ≥0.90 pular, ≥0.70 moderado, <0.70 completo)
    ↓ SkillProposal[] com confiança
Curate (Aceitar/Mesclar/Pular, feedback mascarado do solver)
    ↓ evolved/{skill}/SKILL.md + meta.json
Gate (verificação de formato, dedup, limite 10, promoção com gate ≥ 3 sessões)
    ↓ evolved_backup/ (melhor checkpoint)
Instinct (padrões de alto sucesso → nós cross-project memory.db)
    ↓
Reload (próxima sessão — resume carrega habilidades evoluídas)
```

Semeadura de habilidades: ferramenta fraca (sucesso <60%, mín. 5 obs), tipo de arquivo fraco (sucesso <50%, mín. 3 obs), erro de alta frequência (5+ ocorrências).

Estagnação: 3 sessões sem melhoria de 5% → rollback automático para o melhor checkpoint.

### Efetividade das Habilidades

Cada habilidade evoluída é rastreada com atribuição A/B:

```
/evolve history → Skill Effectiveness

| Skill              | With | Without | Delta |
|--------------------|------|---------|-------|
| evo-ts-care        | 0.87 | 0.72    | +15%  |
| evo-bash-discipline| 0.65 | 0.68    | -3%   |
```

Delta positivo = efetiva. Negativo = considere remover via `/evolve rollback`.

### Presets de Início a Frio

Na primeira sessão, presets de habilidades apropriados para o stack são aplicados automaticamente:

| Stack | Presets |
|-------|---------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

### Aprendizado de Instintos

Padrões de alto sucesso são extraídos e promovidos entre projetos:

```
observe (100% confirmado) → extract_instincts() → instinct node (confiança ≥ 0.8)
    → promover para global quando observado em ≥ 2 projetos
```

```bash
/evolve              # Executar agora
/evolve status       # Painel: pontuações, tendências, padrões, habilidades
/evolve history      # Histórico completo + efetividade das habilidades
/evolve cross-project # Análise de padrões entre projetos
/evolve rollback     # Restaurar melhor versão anterior
/evolve reset        # Limpar todos os dados de evolução
```

---

## Hooks (Ring 0)

Executam de forma invisível em cada sessão. Binário único em Rust (`epic-harness`) com subcomandos.

| Hook | Quando | O que faz |
|------|--------|-----------|
| **resume** | Início da sessão | Restaurar contexto, carregar memória, detectar stack |
| **guard** | Antes de Bash | Bloquear force-push-to-main, `rm -rf /`, DROP em produção |
| **polish** | Após Edit | Autoformatação (Biome/Prettier/ruff/gofmt) + verificação de tipos |
| **observe** | Cada uso de ferramenta | Registrar em `~/.harness/projects/{slug}/obs/` para evolução |
| **snapshot** | Antes de compactar | Salvar estado em `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Fim da sessão | Analisar falhas, semear habilidades evoluídas, gate, extrair instintos |

Polish realimenta observe: falha de formatação → `lint_fail`, erro de TypeScript → `build_fail`. O thrashing Edit→Error é detectado mesmo quando os erros vêm do polish.

Cada sessão grava seu próprio `session_{date}_{pid}_{random}.jsonl` — múltiplas sessões concorrentes não corrompem os dados umas das outras.

### Perfis de Hook

Via `~/.harness/config.toml` ou variável de ambiente `EPIC_HOOK_PROFILE`:

| Perfil | Hooks ativos |
|--------|-------------|
| `minimal` | guard, observe, resume |
| `standard` (padrão) | os anteriores + polish, reflect, snapshot |
| `strict` | todos os hooks + futuras verificações exclusivas de strict |

### Regras de Guard Personalizadas

Adicione regras específicas do projeto via `.harness/guard-rules.yaml` na raiz do seu projeto:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Exclusão de namespace bloqueada
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verifique primeiro
```

---

## Equipe (`epic team`)

As equipes são de **nível organizacional**, não vinculadas ao projeto. Executar `/team` em qualquer projeto enriquece um pool compartilhado de definições de agentes — nunca sobrescreve silenciosamente.

```bash
epic team                              # Interativo: escanear → projetar → escrever → sincronizar
epic team sync backend                 # Despachar agentes → .claude/agents/backend/
epic team link backend                 # Despachar + registrar projeto na configuração da equipe
epic team list                         # Todas as equipes na organização atual
epic team list --org netflix           # Equipes em uma organização nomeada
epic team show backend --playbook      # Configuração + playbook completo
epic team delete backend               # Remover apenas do projeto atual
epic team delete backend --global      # Excluir permanentemente do repositório da organização
```

Após sincronizar, os agentes ficam disponíveis na próxima sessão: `@domain-expert`, `@reviewer`, `@tester`, etc.

| Tipo | Palavra-chave | Agentes padrão |
|------|--------------|----------------|
| Alinhado ao fluxo | `stream` | domain-expert, reviewer, tester |
| Plataforma | `platform` | api-designer, infra-specialist, dx-agent |
| Habilitador | `enabling` | specialist |
| Subsistema complexo | `subsystem` | domain-specialist, integration-tester |

Multi-organização: `epic team --org netflix` — topologia separada por organização.

Estratégia de mesclagem: agentes alterados solicitam confirmação (padrão: manter existente, backup em `.history/`). O playbook sempre é acrescentado.

---

## Suporte Multi-Ferramenta

Todas as ferramentas compartilham o mesmo diretório de dados `~/.harness/projects/{slug}/`.

| Ferramenta | Ring 0 Hooks | Comandos | Habilidades | Agentes |
|------------|-------------|----------|-------------|---------|
| **Claude Code** | ✓ Completo | ✓ 8 comandos (incl. /orbit) | ✓ 11 habilidades | ✓ 4 |
| **Codex CLI** | ✓ Completo¹ | ✓ 8 prompts (incl. /orbit) | ✓ 7 | ✓ 4 |
| **Gemini CLI** | ✓ Parcial² | ✓ 8 comandos (incl. /orbit) | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Completo³ | ✓ 8 comandos (incl. /orbit) | ✓ via rules | ✓ 4 |
| **OpenCode** | ✓ Parcial⁴ | ✓ 8 comandos (incl. /orbit) | — | ✓ 4 |
| **Cline** | ✓ Completo⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ `codex_hooks = true` em `~/.codex/config.toml` · ² Guard no nível `BeforeModel` · ³ Cursor 1.7+ · ⁴ Plugin JS · ⁵ 5 scripts de hook · ⁶ Apenas convenções

---

## Arquitetura: Modelo de 4 Anéis

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
            c1("/discover") --> c2("/spec") --> c3("/go") --> c4("/check") --> c5("/ship") --> c6("/evolve")
        end
        c7("/team")
        c8("/evolve (manual)")
    end

    subgraph R2["Ring 2 — Auto Skills (context-triggered)"]
        direction LR
        s1(tdd) --- s2(debug) --- s3(secure) --- s4(perf) --- s5(simplify) --- s6(verify) --- s7(council)
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

## Aprendizado Entre Projetos

Ative para compartilhar padrões de falha entre projetos:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled
```

Fim da sessão → exporta padrões anonimizados para `~/.harness/global_patterns.jsonl`. Início da sessão → mostra dicas das áreas fracas de outros projetos.

---

## Memória Unificada

Todos os agentes compartilham um grafo de conhecimento em `~/.harness/memory.db` (SQLite com busca de texto completo). Sem runtime externo.

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

### CLI

```bash
epic mem recall "auth refactor" --project my-project   # Recuperação inteligente
epic mem add --title "JWT rotation" --type decision    # Adicionar nó
epic mem search "JWT"                                  # Busca FTS5
epic mem list --type decision --project my-project    # Filtrar
epic mem context --project my-project                  # Contexto do projeto
epic mem serve                                         # Interface Web → :7700 ou porta personalizada com --port 8800
epic mem mcp-install                                   # Registrar servidor MCP
epic mem export --out ./docs/memory                    # Exportar para Markdown
```

### Ferramentas MCP (6)

| Ferramenta | Finalidade |
|------------|-----------|
| `mem_recall` | Recuperação contextual inteligente com hint + projeto + vizinhos do grafo |
| `mem_add` | Adicionar nó com importância automática por tipo (ou explícita 0.0–1.0) |
| `mem_search` | Busca por palavra-chave (texto completo), classificada por importância |
| `mem_query` | Filtrar por tag/tipo/projeto |
| `mem_context` | Recuperação inteligente com escopo de projeto (sem hint) |
| `mem_related` | Percurso do grafo a partir de um ID de nó (encontra conhecimento conectado) |

### Tipos de Nó

| Tipo | Criado por | Importância |
|------|-----------|-------------|
| `decision` | Manual / MCP | 0.9 |
| `resolution` | Manual / MCP | 0.8 |
| `concept` | Manual / MCP | 0.7 |
| `project` | Manual / MCP | 0.7 |
| `instinct` | Auto (reflect) | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

Ciclo de vida: 30+ dias sem acesso → decaimento de 10% na importância (mínimo 0.05). 180+ dias → marcado como `stale`, excluído da recuperação. A tag `pinned` evita o decaimento.

> **Interface Web**: A visualização do grafo está sendo ativamente melhorada — clustering, destaque de vizinhos e fallback offline foram adicionados recentemente. Mais melhorias em andamento.

---

<details>
<summary><strong>Dados do Projeto — layout de diretórios</strong></summary>

## Dados do Projeto

Todos os dados ficam em `~/.harness/` (diretório home), não na raiz do seu projeto. Sobrevive à exclusão do projeto, não polui o histórico do git.

```
~/.harness/
├── memory.db                  # Grafo de conhecimento SQLite (nós + arestas + FTS5)
├── graph.json                 # Grafo em cache (para interface web)
├── config.toml                # Configuração do usuário
├── global_patterns.jsonl      # Padrões entre projetos (opt-in)
├── orgs/                      # Repositório global de equipes
│   └── {org}/teams/{team}/
│       ├── config.json, mission.md, playbook.md, agents/, .history/
└── projects/{slug}/
    ├── memory/                # Padrões e regras do projeto
    ├── sessions/              # Snapshots de sessão (para resume)
    ├── obs/                   # Registros de observação de uso de ferramentas (JSONL)
    ├── evolved/               # Habilidades autoevoluídas
    │   ├── manifest.json
    │   └── {skill}/SKILL.md + meta.json
    ├── evolved_backup/        # Melhor checkpoint (para rollback)
    ├── dispatch/              # Registros de despacho de habilidades
    ├── evolution.jsonl        # Histórico completo de evolução
    └── metrics.json           # Estatísticas agregadas + atribuição de habilidades
```

Compartilhe regras de segurança com sua equipe: `.harness/guard-rules.yaml` na raiz do projeto (commitado no git).

</details>

---

<details>
<summary><strong>Configuração — referência config.toml</strong></summary>

## Configuração

Todos os parâmetros ajustáveis em `~/.harness/config.toml`. Ausente = padrões no código.

```toml
# Prioridade: variável de ambiente (EPIC_HOOK_PROFILE) > este arquivo > padrões

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

## Problemas Conhecidos (Julgamento do Agente)

Esses problemas decorrem da interpretação do contexto pelo agente, e não de bugs no código. Listados aqui para que os usuários saibam o que observar.

### Problemas Descobertos

| Problema | Quando | O que acontece | Solução alternativa |
|----------|--------|----------------|---------------------|
| **Orbit ignora autodificação** | `/orbit` é solicitado a melhorar o próprio orbit | O agente pode ignorar o pipeline do orbit inteiramente e editar arquivos ad-hoc na main, deixando alterações sem commit sem spec/PR/rastreabilidade | Após a conclusão do orbit, verifique `git status`. Se houver alterações na main sem um estado de pipeline, faça commit manualmente ou execute `/orbit` novamente a partir de um branch separado |
| **Tarefa apenas de docs ignora protocolo** | `/orbit` recebe uma alteração apenas de markdown (sem código para testar) | O agente pode julgar as fases TDD/teste como desnecessárias e pular o pipeline completo | Aceitável para alterações puramente de documentação. Para código+documentação mistos, garanta que o agente não pule fases relacionadas a código |
| **Classificação incorreta de modo** | A solicitação está no limite entre Direct e Council | O agente pode escolher Direct quando Council (4 vozes) capturaria mais casos extremos, ou vice-versa | Se o agente escolher um modo que parece incorreto, diga explicitamente "use Council mode" ou "use Direct mode" |

### Escolhas de Design Intencionais

Estas foram consideradas para melhoria, mas mantidas como estão após avaliação:

| Escolha | Por que não foi melhorada | Justificativa |
|---------|--------------------------|---------------|
| **Worktree entra na fase Go, não no início do orbit** | Poderia isolar desde o preflight | Preflight/mode/spec são apenas leitura. Isolar mais cedo adiciona complexidade sem benefício — o branch não é criado até a fase Go de qualquer forma |
| **Worktree preservado após Ship** | Poderia ser removido automaticamente no merge do PR | O branch é a head do PR. Removê-lo antes do merge quebra o PR. A limpeza fica a cargo do usuário após o merge |
| **Branch nomeado `orbit-{slug}` não `feature/{slug}`** | Poderia seguir a nomenclatura convencional de branches | `EnterWorktree` não permite `/` nos nomes. Renomear após a criação adiciona uma etapa apenas para benefício cosmético |
| **Sem pipeline leve para alterações de documentação** | Poderia detectar apenas docs e pular TDD/testes | A detecção é frágil (o que conta como "doc"?). Adicionar um caminho separado aumenta a complexidade do protocolo por ganho marginal |

---

## Solução de Problemas

<details>
<summary>command not found: epic após instalação</summary>

Adicione o diretório bin do Cargo ao seu PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Adicione esta linha ao seu `~/.zshrc` ou `~/.bashrc` para torná-la permanente.
</details>

<details>
<summary>Hooks não estão sendo executados no Claude Code</summary>

Execute a instalação novamente para sincronizar os hooks nas configurações do Claude Code:

```bash
epic install claude
```

Depois reinicie o Claude Code. Os hooks são gravados em `~/.claude/settings.json`.
</details>

<details>
<summary>Permission denied no macOS (Gatekeeper)</summary>

O macOS pode bloquear binários não assinados baixados da internet:

```bash
xattr -d com.apple.quarantine ~/.cargo/bin/epic-harness
xattr -d com.apple.quarantine ~/.cargo/bin/epic
```
</details>

<details>
<summary>epic: binário não encontrado nos hooks do plugin</summary>

O plugin procura o binário em `hooks/bin/epic-harness` primeiro. Após atualizar via `cargo install`, copie-o:

```bash
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness
```
</details>

---

## Desenvolvimento

```bash
cargo install --path .                                        # Compilar + instalar
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness           # Atualizar binário do plugin
cargo test                                                    # Testes
```

Os hooks procuram o binário em dois lugares: `hooks/bin/epic-harness` (plugin local) → `~/.cargo/bin/epic-harness` (PATH).

---

## Links

- [Changelog](../../CHANGELOG.md) — histórico de versões
- [Contribuindo](../../CONTRIBUTING.md) — como contribuir
- [Segurança](../../SECURITY.md) — relatar vulnerabilidades
- [Issues](https://github.com/epicsagas/epic-harness/issues) — relatórios de bugs e solicitações de funcionalidades

## Agradecimentos

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Padrões de evolução automatizada e benchmarks
- [agent-skills](https://github.com/addyosmani/agent-skills) — Sistema de habilidades de agente do Claude Code
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Padrões abrangentes do Claude Code
- [gstack](https://github.com/garrytan/gstack) — Referência de arquitetura de plugins
- [harness](https://github.com/revfactory/harness) — Padrões de infraestrutura de hooks e harness
- [serena](https://github.com/oraios/serena) — Design de agentes autônomos
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Arquitetura de framework multi-comando
- [superpowers](https://github.com/obra/superpowers) — Padrões de extensão do Claude Code

## Licença

[Apache 2.0](../../LICENSE)
