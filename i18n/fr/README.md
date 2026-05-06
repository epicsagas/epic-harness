# epic harness

**6 commandes. Compétences à déclenchement automatique. Auto-évolutif.**

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

Un plugin Claude Code qui **remplace plus de 30 commandes par 6**, **déclenche automatiquement des compétences** en fonction de ce que vous faites, et **fait évoluer de nouvelles compétences** à partir de vos propres schémas d'échec. Moins de surface à mémoriser. Plus d'intelligence par frappe.

<p align="center">
  <img src="../../assets/features.jpg" alt="fonctionnalités epic harness" width="100%" />
</p>

## Architecture : modèle à 4 anneaux

```
Ring 0 — Pilote automatique (hooks, invisible)
  Restauration de session, formatage auto, garde-fous, journalisation des observations

Ring 1 — 6 commandes (vous les appelez)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — Compétences automatiques (déclenchées par le contexte)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — Évolution (auto-amélioration)
  Observer l'utilisation des outils → analyser les échecs → générer des compétences → contrôle → rechargement
```

## Installation

```
# Plugin Claude Code (recommandé)
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# Ou depuis les sources
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### Installer depuis le binaire

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# Depuis crates.io
cargo install epic-harness

# Binaire pré-compilé (plus rapide, sans compilation)
cargo binstall epic-harness

# Depuis les sources
cargo install --path .
```

Le binaire est automatiquement détecté par les hooks. S'il est absent, les hooks reviennent à Node.js.

## Support multi-outils

epic-harness fonctionne avec Claude Code et 6 autres outils de codage IA. Tous les outils partagent le même répertoire de données `~/.harness/projects/{slug}/`.

| Outil | Ring 0 Hooks | Commandes/Prompts | Compétences | Agents |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Complet | ✓ 6 commandes | ✓ 10 compétences | ✓ 4 |
| **Codex CLI** | ✓ Complet¹ | ✓ 6 prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Partiel² | ✓ 6 commandes | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Complet³ | ✓ 6 commandes | ✓ via règles | ✓ 4 |
| **OpenCode** | ✓ Partiel⁴ | ✓ 6 commandes | — | ✓ 4 |
| **Cline** | ✓ Complet⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Nécessite `codex_hooks = true` dans `~/.codex/config.toml` ; PostToolUse intercepte uniquement Bash
² Pas d'équivalent `PreToolUse` — guard s'exécute au niveau `BeforeModel`
³ Nécessite Cursor 1.7+
⁴ Plugin JS : `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ Scripts de hook PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel
⁶ Pas de système de hooks — conventions injectées via `.aider/CONVENTIONS.md` + `.aider.conf.yml`

### Installer pour d'autres outils

```bash
# Menu interactif (sélectionner les outils à installer)
epic install

# Installation directe
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (nécessite Cursor 1.7+)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Installation locale au projet
epic install cursor --local

# Aperçu sans effectuer de changements
epic install gemini --dry-run
```

Les fichiers d'intégration dans le répertoire de l'outil (`hooks.json`, commandes, agents, compétences, règles…) sont **synchronisés** depuis le binaire : les fichiers manquants ou obsolètes sont écrits. `GEMINI.md` et `AGENTS.md` ne sont créés que s'ils sont absents.

## Mémoire unifiée

Tous les agents partagent un graphe de connaissances unique stocké dans `~/.harness/memory.db` (SQLite + FTS5). Aucun Node.js ni runtime externe requis.

### Rappel intelligent

La récupération de mémoire utilise un **score composite** plutôt que de déverser les N dernières entrées :

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **Importance** définie automatiquement par type de nœud : decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **Suivi des accès** : les mémoires fréquemment rappelées remontent naturellement
- **Décroissance progressive** : les mémoires inutilisées perdent en importance au fil du temps (10% tous les 30 jours, plancher 0.05)
- **Augmentation du graphe** : le rappel suit les arêtes à 1 saut pour faire émerger le contexte connexe

### CLI

```bash
# Rappel intelligent — classé par pertinence pour votre tâche actuelle
epic mem recall "auth refactor" --project my-project

# Ajouter un nœud de mémoire (importance auto par type, ou explicite)
epic mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
epic mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# Requête filtrée (inclut importance + access_count)
epic mem query --type decision --project my-project

# Recherche plein texte (classée par importance)
epic mem search "JWT"

# Contexte intelligent (pondéré par importance, pas seulement le plus récent)
epic mem context --project my-project

# Interface Web du graphe de connaissances
epic mem serve          # → http://localhost:7700

# Enregistrer comme serveur MCP dans Claude Code (sans Node.js)
epic mem mcp-install

# Exporter tous les nœuds en Markdown pour sauvegarde Git
epic mem export --out ./docs/memory
```

### Outils MCP (6)

Lorsqu'enregistré comme serveur MCP (`epic mem mcp-install`), les agents peuvent appeler directement ces outils :

| Outil | Objectif |
|------|---------|
| `mem_recall` | **Principal.** Rappel contextuel intelligent avec hint + projet + voisins du graphe |
| `mem_add` | Ajouter un nœud avec importance auto par type (ou explicite 0.0–1.0) |
| `mem_search` | Recherche FTS5, résultats classés par importance |
| `mem_query` | Filtrer par tag/type/projet |
| `mem_context` | Rappel intelligent limité au projet (sans hint) |
| `mem_related` | Traversée BFS du graphe depuis un ID de nœud |

### Comment fonctionne le graphe de connaissances

Le graphe s'accumule automatiquement à partir du travail normal de session — aucune saisie manuelle nécessaire.

**Flux de données :**

```
PostToolUse hook → observe (notation sur 3 axes) → obs/*.jsonl
                                                         ↓
SessionEnd hook → reflect (détection de patterns) → nœuds + arêtes memory.db
                                                         ↓  (importance définie par type)
SessionStart hook → resume (rappel intelligent) → la prochaine session reçoit des hints classés par pertinence
                              ↓
                    decay_importance() → les nœuds inutilisés s'estompent progressivement
```

**Types de nœuds (7) :**

| Type | Créé par | Importance par défaut |
|------|-----------|-------------------|
| `decision` | Manuel / MCP | 0.9 |
| `resolution` | Manuel / MCP | 0.8 |
| `concept` | Manuel / MCP | 0.7 |
| `project` | Manuel / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**Cycle de vie de la mémoire :**

| Événement | Ce qui se passe |
|-------|-------------|
| Nœud rappelé via search/recall/context | `access_count++`, `accessed_at` mis à jour |
| 30+ jours sans accès | importance décroît de 10% (plancher 0.05) |
| 180+ jours sans accès | tagué `stale`, exclu du rappel |
| Nœud tagué `pinned` | immunisé contre la décroissance |

**Conditions d'accumulation automatique :**

| Condition | Nœud créé |
|-----------|-------------|
| Chaque fin de session | `session` (toujours) |
| Même erreur ≥3 fois de suite | `error` (repeated_same_error) |
| Edit→Error en alternance | `pattern` (thrashing) |
| Taux de réussite de l'outil <60% (min. 5 observations) | `pattern` (weak_tool) |
| Taux de réussite du type de fichier <50% (min. 3 observations) | `pattern` (weak_filetype) |
| Cycles de succès Edit → erreur Bash | `pattern` (fix_then_break) |

> **Note :** Les sessions propres (sans erreurs) ne produisent que des nœuds `session`. Le graphe s'enrichit après 2–3 sessions de développement réelles avec des échecs de build, des échecs de tests ou des cycles de débogage.

Les mémoires existantes basées sur des fichiers (`nodes/*.md`, `edges.jsonl`) sont automatiquement migrées vers SQLite lors de la première exécution.

## Commandes

| Commande | Ce qu'elle fait |
|----------|----------------|
| `/spec` | Définir ce qu'il faut construire — clarifier les exigences, produire une spécification |
| `/go` | Construire — planification auto, sous-agents TDD, exécution parallèle |
| `/check` | Vérifier — revue de code parallèle + audit de sécurité + performance |
| `/ship` | Livrer — PR, CI, merge |
| `/team` | Créer et synchroniser des équipes d'agents au niveau de l'organisation entre les projets |
| `/evolve` | Déclenchement manuel de l'évolution / statut / rollback |

## Équipes (`epic team`)

Les équipes sont **au niveau de l'organisation**, pas liées à un projet. L'exécution de `/team` dans n'importe quel projet enrichit un pool partagé de définitions d'agents — sans jamais écraser silencieusement.

### Comment ça fonctionne

```
epic team                      # interactif : scanner le projet → concevoir → écrire → synchroniser
         ↓
~/.harness/orgs/epic/teams/backend/   ← magasin global (persiste entre les projets)
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code découvre automatiquement au démarrage de session
├── domain-expert.md                  ← définition de rôle + contexte d'équipe injecté
├── reviewer.md
└── tester.md
         ↓
Session suivante : agents actifs — sélectionnés automatiquement par Claude ou appelés explicitement
```

### Référence CLI

```bash
# Créer ou mettre à jour une équipe (flux interactif en 4 phases)
epic team

# Parcourir
epic team list                        # toutes les équipes dans l'org actuelle
epic team list --org netflix          # équipes dans un org nommé
epic team show backend                # config, mission, agents
epic team show backend --playbook     # + playbook accumulé complet

# Déployer vers un projet
epic team sync backend                # déployer : copier les agents → .claude/agents/backend/
epic team link backend                # déployer + enregistrer le projet dans la config de l'équipe

# Rappeler depuis un projet
epic team delete backend              # rappeler : retirer du projet actuel uniquement
epic team unlink backend              # alias pour delete

# Dissoudre (retirer complètement de l'org)
epic team delete backend --global     # supprimer définitivement du magasin org + copie locale

# Historique
epic team history backend reviewer    # lister les sauvegardes .history/ pour un agent
```

### Utiliser les équipes depuis les agents de codage

Après synchronisation, les agents sont disponibles automatiquement dans la session suivante :

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert implémenter la passerelle de paiement
@reviewer vérifier ce PR pour les cas limites
@tester écrire des tests d'intégration pour auth

# Ou laisser l'agent sélectionner automatiquement selon le contexte de la tâche
```

Chaque fichier d'agent porte une section **Contexte d'équipe** injectée à la synchronisation :

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

Les agents connaissent leur équipe, mission, et comment charger le playbook complet à la demande —
sans surcharger la fenêtre de contexte.

### Multi-org

```bash
epic team                          # accumule dans l'org "epic" (par défaut)
epic team --org netflix            # topologie Netflix séparée
epic team --org client-x           # engagement par client
```

Même nom d'équipe dans le même org = partage intentionnel entre projets.
`epic/teams/backend` accumule les connaissances de chaque projet qui le crée ou le lie.

### Types d'équipes

| Type | Mot-clé | Agents par défaut |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### Stratégie de fusion — pas d'écrasement silencieux

| Objet | Règle |
|--------|------|
| Agent — nouveau | Ajout automatique |
| Agent — inchangé | Ignoré |
| Agent — modifié | **Invite** (par défaut : conserver l'existant). En cas de remplacement → sauvegardé dans `.history/` |
| `playbook.md` | Toujours **ajouté** — jamais tronqué |
| `mission.md` — modifié | **Invite** (par défaut : conserver l'existant) |

## Compétences automatiques (Ring 2)

Les compétences se déclenchent automatiquement en fonction du contexte. Vous n'avez pas besoin de les invoquer.

| Compétence | Se déclenche quand |
|------------|-------------------|
| **tdd** | Implémentation d'une nouvelle fonctionnalité |
| **debug** | Échec de test ou erreur |
| **secure** | Code d'authentification/BDD/API/secrets modifié |
| **perf** | Boucles, requêtes, code de rendu |
| **simplify** | Fichier > 200 lignes ou complexité élevée |
| **document** | API publique ajoutée ou modifiée |
| **verify** | Avant de terminer /go ou /ship |
| **context** | Fenêtre de contexte > 70 % utilisée |

## Hooks (Ring 0)

S'exécutent de manière invisible. Aucune action utilisateur requise. Implémentés sous forme d'un **unique binaire Rust** (`epic-harness`) avec des sous-commandes, avec repli sur Node.js si le binaire est absent.

```
epic resume | guard | polish | observe | snapshot | reflect
```

| Hook | Quand | Action |
|------|-------|--------|
| **resume** | Début de session | Restaurer le contexte, charger la mémoire, détecter la stack |
| **guard** | Avant Bash | Bloquer force-push-to-main, rm -rf /, DROP prod |
| **polish** | Après Edit | Formatage auto (Biome/Prettier/ruff/gofmt) + vérification de types |
| **observe** | Chaque utilisation d'outil | Journaliser dans `~/.harness/projects/{slug}/obs/` pour l'évolution |
| **snapshot** | Avant compactage | Sauvegarder l'état dans `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Fin de session | Analyser les échecs, générer des compétences évoluées, contrôle |

## Système d'évaluation (noyau du Ring 3)

Fusionne les patterns de benchmark d'A-Evolve dans le système de hooks de Claude Code.

### Notation multi-dimensionnelle

Chaque appel d'outil est noté sur 3 axes. Les pondérations sont configurables via `SCORE_WEIGHTS` dans `~/.harness/config.toml` :

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (défaut : 0.5)                          (défaut : 0.3)                             (défaut : 0.2)
```

| Dimension | Ce qu'elle mesure | Critères par outil |
|-----------|------------------|-------------------|
| `tool_success` | A-t-il fonctionné ? (0/1) | Classification d'échec en 9 catégories |
| `output_quality` | Signaux de qualité de sortie (0.0-1.0) | Bash : avertissements, sortie vide. Edit : détection de ré-édition |
| `execution_cost` | Indicateur d'efficacité (0.0-1.0) | Taille de sortie, liste blanche de commandes silencieuses |

### Classification des échecs (9 catégories)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Détection de patterns (4 types)

Tous les seuils sont des constantes configurables dans `~/.harness/config.toml` :

| Pattern | Détecte | Constante | Défaut |
|---------|---------|-----------|--------|
| `repeated_same_error` | Même erreur N+ fois de suite | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Édition réussie → échec build/test | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Bloqué sur le même fichier N+ opérations | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Alternance Édition↔Erreur sur le même fichier | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Seuils de génération de compétences

| Déclencheur | Constante | Défaut |
|-------------|-----------|--------|
| Outil faible (faible taux de succès) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| Type de fichier faible | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| Erreur haute fréquence | `HIGH_FREQ_ERROR_MIN` | 5 |

### Contrôle de stagnation

- `STAGNATION_LIMIT` (défaut : 3) sessions sans amélioration → rollback automatique des compétences évoluées vers le meilleur point de contrôle
- `IMPROVEMENT_THRESHOLD` (défaut : 5 %)
- Suivi de tendance : `improving` / `stable` / `declining` via régression linéaire
- Les compétences statiques ont toujours la priorité sur les compétences évoluées en cas de conflit

### Flux d'évolution

```
Observer (PostToolUse — notation sur 3 axes)
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyser (SessionEnd)
    ↓ SessionAnalysis : par outil, par extension, distribution des scores
    ↓ Patterns : repeated_same_error, fix_then_break, long_debug_loop, thrashing
Générer (4 voies : pattern / outil faible / type de fichier faible / erreur haute fréquence)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Contrôler (vérification de format, déduplication, limite de 10, vérification de stagnation)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (meilleur point de contrôle)
Recharger (session suivante — resume.ts rapporte les métriques + charge les compétences évoluées)
```

```bash
/evolve              # Lancer l'évolution maintenant
/evolve status       # Tableau de bord : scores, tendances, patterns, compétences
/evolve history      # Analyse long terme : historique complet, efficacité des compétences, stats de dispatch
/evolve cross-project # Analyse de patterns inter-projets
/evolve rollback     # Restaurer le meilleur état précédent
/evolve reset        # Effacer toutes les données d'évolution
```

## Préréglages de démarrage à froid

Pas besoin d'attendre 5 sessions pour obtenir des compétences évoluées utiles. Dès la première session, epic harness détecte votre stack et applique automatiquement des compétences prédéfinies :

| Stack | Compétences prédéfinies |
|-------|------------------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Les préréglages sont des compléments — ils sont remplacés par de véritables compétences évoluées au fur et à mesure que les données s'accumulent.

## Sécurité des sessions concurrentes

Chaque session écrit dans son propre fichier d'observation (`session_{date}_{pid}_{random}.jsonl`). Plusieurs sessions Claude Code sur le même projet ne corrompront pas les données des autres. Le hook reflect fusionne tous les fichiers du même jour pour l'analyse.

## Règles de garde personnalisées

Ajoutez des règles de sécurité spécifiques au projet via `.harness/guard-rules.yaml` à la racine du projet :

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Les règles fusionnent avec les gardes intégrées (force-push-to-main, rm -rf /, DROP prod). Conserver ce fichier dans git permet de partager les règles de sécurité avec votre équipe.

## Apprentissage inter-projets

Activez le partage de patterns d'échec entre projets :

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # opt-in
```

Lorsqu'activé :
- La fin de session exporte des patterns anonymisés vers `~/.harness/global_patterns.jsonl`
- Le début de session affiche des indices provenant des faiblesses d'autres projets
- Utilisez `/evolve cross-project` pour voir les patterns agrégés

## Suivi de l'efficacité des compétences

Chaque compétence évoluée est suivie avec des scores d'attribution A/B :

```
/evolve history → Section Efficacité des compétences

| Compétence         | Sessions | Score avec | Score sans    | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

Un delta positif = la compétence aide. Un delta négatif = envisagez de la supprimer via `/evolve rollback`.

## Retour Polish → Observe

Le hook polish (formatage auto + vérification de types) réinjecte ses résultats dans le pipeline d'observation :

- Échec de formatage → enregistré comme `lint_fail`
- Erreur TypeScript → enregistrée comme `build_fail`
- Succès → enregistrés avec les scores complets

Cela signifie que les patterns de thrashing « édition → erreur de type → édition → erreur de type » sont détectés même lorsque les erreurs proviennent du hook polish et non de commandes manuelles.

## Données du projet (`~/.harness/projects/{slug}/`)

Les données spécifiques au projet résident dans votre répertoire home. Elles survivent à la suppression du projet et ne polluent pas l'historique git.

```
~/.harness/projects/{slug}/
├── memory/           # Patterns et règles du projet (persistant)
├── sessions/         # Instantanés de session (pour la restauration)
├── obs/              # Journaux d'observation d'utilisation des outils (JSONL, par session)
├── evolved/          # Compétences auto-évoluées
├── evolved_backup/   # Meilleur point de contrôle (pour rollback en cas de stagnation)
├── dispatch/         # Journaux de dispatch des compétences (JSONL)
├── team/             # legacy (remplacé par ~/.harness/orgs/)
├── evolution.jsonl   # Historique complet de l'évolution
└── metrics.json      # Statistiques agrégées + attribution des compétences

~/.harness/
├── memory.db         # Graphe de connaissances SQLite (nœuds + arêtes + FTS5)
├── graph.json        # Graphe mis en cache (pour l'interface Web)
└── orgs/             # Magasin global epic team
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

Vous pouvez toujours utiliser `.harness/guard-rules.yaml` à la racine du projet pour partager des règles de sécurité avec votre équipe.

## Développement

### Build

```bash
cargo install --path .          # Compiler + installer dans ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Mettre à jour le binaire du plugin
```

### Dispatching des hooks

Chaque hook dans `hooks.json` cherche le binaire Rust à deux emplacements :

```
1. Local au plugin : hooks/bin/epic-harness
2. PATH :           ~/.cargo/bin/epic-harness (via cargo install)
```

### Tests

```bash
cargo test       # Tests unitaires + d'intégration Rust
```

## Remerciements

epic harness a été inspiré par et construit à partir d'idées issues des projets suivants :

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Patterns d'évolution automatisée et de benchmark
- [agent-skills](https://github.com/addyosmani/agent-skills) — Système de compétences d'agent Claude Code
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Patterns Claude Code complets
- [gstack](https://github.com/garrytan/gstack) — Référence d'architecture de plugin
- [harness](https://github.com/revfactory/harness) — Patterns d'infrastructure de hooks et harness
- [serena](https://github.com/oraios/serena) — Conception d'agent autonome
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Architecture de framework multi-commandes
- [superpowers](https://github.com/obra/superpowers) — Patterns d'extension Claude Code

## Licence

[Apache 2.0](LICENSE)
