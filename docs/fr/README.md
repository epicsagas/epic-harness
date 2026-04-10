# epic harness

**6 commandes. Compétences à déclenchement automatique. Auto-évolutif.**

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

```bash
# Marketplace du plugin Claude Code
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# Ou manuellement
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Binaire Rust (optionnel, hooks ~4x plus rapides)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# Depuis crates.io
cargo install epic-harness
# ou avec cargo-binstall (pré-compilé, plus rapide)
cargo binstall epic-harness

# Depuis les sources
cargo install --path .
```

Le binaire est automatiquement détecté par les hooks. S'il est absent, les hooks se rabattent sur Node.js.

## Support multi-outils

epic-harness fonctionne avec Claude Code et 6 autres outils de codage IA. Tous les outils partagent le même répertoire de données `~/.harness/projects/{slug}/`.

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

### Installer pour d'autres outils

```bash
# Menu interactif
epic-harness install

# Installation directe
epic-harness install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic-harness install gemini       # Gemini CLI  → ~/.gemini/
epic-harness install cursor       # Cursor      → ~/.cursor/ (requires Cursor 1.7+)
epic-harness install opencode     # OpenCode    → ~/.config/opencode/
epic-harness install cline        # Cline       → ~/Documents/Cline/Rules/
epic-harness install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Installer localement au projet
epic-harness install cursor --local

# Aperçu sans effectuer de changements
epic-harness install gemini --dry-run
```

## Commandes

| Commande | Ce qu'elle fait |
|----------|----------------|
| `/spec` | Définir ce qu'il faut construire — clarifier les exigences, produire une spécification |
| `/go` | Construire — planification auto, sous-agents TDD, exécution parallèle |
| `/check` | Vérifier — revue de code parallèle + audit de sécurité + performance |
| `/ship` | Livrer — PR, CI, merge |
| `/team` | Concevoir une équipe d'agents spécifique au projet |
| `/evolve` | Déclenchement manuel de l'évolution / statut / rollback |

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

S'exécutent de manière invisible. Aucune action utilisateur requise. Implémentés sous forme d'un **unique binaire Rust** (`epic-harness`) avec des sous-commandes, se rabattant sur Node.js si le binaire n'est pas disponible.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| Hook | Quand | Action |
|------|-------|--------|
| **resume** | Début de session | Restaurer le contexte, charger la mémoire, détecter la stack |
| **guard** | Avant Bash | Bloquer force-push-to-main, rm -rf /, DROP prod |
| **polish** | Après Edit | Formatage auto (Biome/Prettier/ruff/gofmt) + vérification de types |
| **observe** | Chaque utilisation d'outil | Journaliser dans `.harness/obs/` pour l'évolution |
| **snapshot** | Avant compactage | Sauvegarder l'état dans `.harness/sessions/` |
| **reflect** | Fin de session | Analyser les échecs, générer des compétences évoluées, contrôle |

## Système d'évaluation (noyau du Ring 3)

Fusionne les patterns de benchmark d'A-Evolve dans le système de hooks de Claude Code.

### Notation multi-dimensionnelle

Chaque appel d'outil est noté sur 3 axes. Les pondérations sont configurables via `SCORE_WEIGHTS` dans `src/ts/common.ts` (ou `src/hooks/common.rs`) :

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

Tous les seuils sont des constantes configurables dans `src/ts/common.ts` (ou `src/hooks/common.rs`) :

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
    ↓ .harness/obs/session_{id}.jsonl
Analyser (SessionEnd)
    ↓ SessionAnalysis : par outil, par extension, distribution des scores
    ↓ Patterns : repeated_same_error, fix_then_break, long_debug_loop, thrashing
Générer (4 voies : pattern / outil faible / type de fichier faible / erreur haute fréquence)
    ↓ .harness/evolved/{skill}/SKILL.md
Contrôler (vérification de format, déduplication, limite de 10, vérification de stagnation)
    ↓ .harness/evolved_backup/ (meilleur point de contrôle)
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

Ajoutez des règles de sécurité spécifiques au projet via `.harness/guard-rules.yaml` :

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Les règles fusionnent avec les gardes intégrées (force-push-to-main, rm -rf /, DROP prod).

## Apprentissage inter-projets

Activez le partage de patterns d'échec entre projets :

```bash
touch .harness/.cross-project-enabled  # opt-in
```

Lorsqu'activé :
- La fin de session exporte des patterns anonymisés vers `~/.harness-global/patterns.jsonl`
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

## Données du projet (`.harness/`)

epic harness crée un répertoire `.harness/` dans votre projet :

```
.harness/
├── memory/           # Patterns et règles du projet (persistant)
├── sessions/         # Instantanés de session (pour la restauration)
├── obs/              # Journaux d'observation d'utilisation des outils (JSONL, par session)
├── evolved/          # Compétences auto-évoluées
├── evolved_backup/   # Meilleur point de contrôle (pour rollback en cas de stagnation)
├── dispatch/         # Journaux de dispatch des compétences (JSONL)
├── team/             # Agents et compétences générés par /team
├── evolution.jsonl   # Historique complet de l'évolution
├── metrics.json      # Statistiques agrégées + attribution des compétences
└── guard-rules.yaml  # Règles de garde personnalisées (optionnel)
```

Ajoutez `.harness/` à `.gitignore` ou committez-le — c'est votre choix.

## Développement

### Rust (principal — ~4x plus rapide)

```bash
cargo install --path .          # Compiler + installer dans ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Mettre à jour le binaire du plugin
```

### Node.js (solution de repli)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### Dispatching des hooks

Chaque hook dans `hooks.json` cherche le binaire Rust à trois emplacements, puis se rabat sur Node.js :

```
1. Local au plugin : hooks/bin/epic-harness
2. PATH :           ~/.cargo/bin/epic-harness (via cargo install)
3. Repli :          node hooks/scripts/<hook>.js
```

### Tests

```bash
cargo test       # 98 tests unitaires Rust
npm test         # Tests unitaires + e2e Node.js
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
