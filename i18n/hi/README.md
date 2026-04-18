# epic harness

**6 कमांड। ऑटो-ट्रिगर स्किल्स। स्वयं-विकसित होने वाला।**

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

एक Claude Code प्लगइन जो **30+ कमांड को 6 से बदल देता है**, आप जो कर रहे हैं उसके आधार पर **स्किल्स स्वचालित रूप से ट्रिगर करता है**, और आपकी विफलता पैटर्न से **नई स्किल्स विकसित करता है**। याद रखने के लिए कम सतह क्षेत्र। प्रत्येक कीस्ट्रोक में अधिक बुद्धिमत्ता।

<p align="center">
  <img src="../../assets/features.jpg" alt="epic harness features" width="100%" />
</p>

## आर्किटेक्चर: 4-रिंग मॉडल

```
Ring 0 — ऑटोपायलट (हुक्स, अदृश्य)
  सेशन रिस्टोर, ऑटो-फॉर्मेट, गार्ड रेल्स, ऑब्ज़र्वेशन लॉगिंग

Ring 1 — 6 कमांड (आप इन्हें कॉल करते हैं)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — ऑटो स्किल्स (संदर्भ-ट्रिगर)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — इवॉल्व (स्वयं-सुधार करने वाला)
  टूल उपयोग का अवलोकन → विफलताओं का विश्लेषण → स्किल्स का स्वतः निर्माण → गेट → रीलोड
```

## इंस्टॉल करें

```
# Claude Code प्लगइन (अनुशंसित)
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

```bash
# या सोर्स से
git clone https://github.com/epicsagas/epic-harness.git
cd epic-harness
cargo install --path .
epic install
```

### बाइनरी से इंस्टॉल करें

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# crates.io से
cargo install epic-harness

# पूर्व-निर्मित बाइनरी (तेज़, बिना कम्पाइल)
cargo binstall epic-harness

# सोर्स से
cargo install --path .
```

बाइनरी हुक्स द्वारा स्वचालित रूप से पहचानी जाती है। अनुपस्थित होने पर हुक्स Node.js पर फ़ॉलबैक करते हैं।

## मल्टी-टूल सपोर्ट

epic-harness Claude Code और 6 अतिरिक्त AI कोडिंग टूल्स के साथ काम करता है। सभी टूल्स एक ही `~/.harness/projects/{slug}/` डेटा डायरेक्टरी साझा करते हैं।

| टूल | Ring 0 Hooks | कमांड/प्रॉम्प्ट | स्किल्स | एजेंट्स |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ पूर्ण | ✓ 6 कमांड | ✓ 8 स्किल्स | ✓ 4 |
| **Codex CLI** | ✓ पूर्ण¹ | ✓ 6 प्रॉम्प्ट | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ आंशिक² | ✓ 6 कमांड | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ पूर्ण³ | ✓ 6 कमांड | ✓ नियमों के माध्यम से | ✓ 4 |
| **OpenCode** | ✓ आंशिक⁴ | ✓ 6 कमांड | — | ✓ 4 |
| **Cline** | ✓ पूर्ण⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ `~/.codex/config.toml` में `codex_hooks = true` आवश्यक; PostToolUse केवल Bash को इंटरसेप्ट करता है
² कोई `PreToolUse` समकक्ष नहीं — guard `BeforeModel` स्तर पर चलता है
³ Cursor 1.7+ आवश्यक
⁴ JS प्लगइन: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel हुक स्क्रिप्ट
⁶ कोई हुक सिस्टम नहीं — कन्वेंशन `.aider/CONVENTIONS.md` + `.aider.conf.yml` के माध्यम से इंजेक्ट

### अन्य टूल्स के लिए इंस्टॉल करें

```bash
# इंटरएक्टिव मेनू (इंस्टॉल करने के लिए टूल्स चुनें)
epic install

# सीधे इंस्टॉल करें
epic install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic install gemini       # Gemini CLI  → ~/.gemini/
epic install cursor       # Cursor      → ~/.cursor/ (Cursor 1.7+ आवश्यक)
epic install opencode     # OpenCode    → ~/.config/opencode/
epic install cline        # Cline       → ~/Documents/Cline/Rules/
epic install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# प्रोजेक्ट-लोकल इंस्टॉल करें
epic install cursor --local

# बिना बदलाव किए पूर्वावलोकन
epic install gemini --dry-run
```

टूल डायरेक्टरी में इंटीग्रेशन फ़ाइलें (`hooks.json`, कमांड, एजेंट, स्किल्स, नियम, …) बाइनरी से **सिंक** की जाती हैं: गायब या पुरानी फ़ाइलें लिखी जाती हैं। `GEMINI.md` और `AGENTS.md` केवल तभी बनाए जाते हैं जब अनुपस्थित हों।

## एकीकृत मेमोरी

सभी एजेंट `~/.harness/memory.db` (SQLite + FTS5) में संग्रहीत एक साझा नॉलेज ग्राफ़ उपयोग करते हैं। कोई Node.js या बाहरी रनटाइम आवश्यक नहीं।

### स्मार्ट रिकॉल

मेमोरी पुनर्प्राप्ति नवीनतम N प्रविष्टियों को डंप करने के बजाय **कम्पोज़िट स्कोरिंग** का उपयोग करती है:

```
score = recency(25%) + importance(35%) + access_frequency(15%) + FTS_match(25%)
```

- **महत्व** नोड प्रकार द्वारा स्वतः सेट: decision(0.9) > resolution(0.8) > concept(0.7) > pattern(0.5) > error(0.4) > session(0.2)
- **एक्सेस ट्रैकिंग**: बार-बार रिकॉल की गई मेमोरी स्वाभाविक रूप से ऊपर आती हैं
- **क्रमिक क्षय**: अप्रयुक्त मेमोरी समय के साथ महत्व खोती हैं (हर 30 दिन में 10%, न्यूनतम 0.05)
- **ग्राफ़ वृद्धि**: रिकॉल संबंधित संदर्भ लाने के लिए 1-हॉप एज का अनुसरण करता है

### CLI

```bash
# स्मार्ट रिकॉल — आपके वर्तमान कार्य के लिए प्रासंगिकता-रैंक
harness mem recall "auth refactor" --project my-project

# मेमोरी नोड जोड़ें (महत्व प्रकार द्वारा स्वतः, या स्पष्ट)
harness mem add --title "JWT rotation strategy" --type decision --tags auth --body "..."
harness mem add --title "Custom pattern" --type concept --importance 0.8 --body "..."

# फ़िल्टर क्वेरी (महत्व + access_count सहित)
harness mem query --type decision --project my-project

# फ़ुल-टेक्स्ट खोज (महत्व द्वारा रैंक)
harness mem search "JWT"

# स्मार्ट कॉन्टेक्स्ट (महत्व-भारित, केवल नवीनतम नहीं)
harness mem context --project my-project

# नॉलेज ग्राफ़ वेब UI
harness mem serve          # → http://localhost:7700

# Claude Code में MCP सर्वर के रूप में पंजीकृत करें (Node.js आवश्यक नहीं)
harness mem mcp-install

# Git बैकअप के लिए सभी नोड्स को Markdown में निर्यात करें
harness mem export --out ./docs/memory
```

### MCP टूल्स (6)

MCP सर्वर के रूप में पंजीकृत होने पर (`harness mem mcp-install`), एजेंट इन टूल्स को सीधे कॉल कर सकते हैं:

| टूल | उद्देश्य |
|------|---------|
| `mem_recall` | **प्राथमिक।** hint + project + graph पड़ोसियों के साथ स्मार्ट संदर्भ रिकॉल |
| `mem_add` | प्रकार द्वारा ऑटो-महत्व के साथ नोड जोड़ें (या स्पष्ट 0.0–1.0) |
| `mem_search` | FTS5 कीवर्ड खोज, महत्व द्वारा परिणाम रैंक |
| `mem_query` | टैग/प्रकार/प्रोजेक्ट द्वारा फ़िल्टर करें |
| `mem_context` | प्रोजेक्ट-स्कोप्ड स्मार्ट रिकॉल (कोई hint नहीं) |
| `mem_related` | नोड ID से BFS ग्राफ़ ट्रैवर्सल |

### नॉलेज ग्राफ़ कैसे काम करता है

ग्राफ़ सामान्य सेशन कार्य से स्वचालित रूप से संचित होता है — किसी मैन्युअल इनपुट की आवश्यकता नहीं।

**डेटा प्रवाह:**

```
PostToolUse hook → observe (3-अक्ष स्कोरिंग) → obs/*.jsonl
                                                        ↓
SessionEnd hook → reflect (पैटर्न पहचान) → memory.db नोड्स + एज
                                                        ↓  (महत्व प्रकार द्वारा सेट)
SessionStart hook → resume (स्मार्ट रिकॉल) → अगले सेशन को प्रासंगिकता-रैंक hints मिलते हैं
                              ↓
                    decay_importance() → अप्रयुक्त नोड्स धीरे-धीरे फीके पड़ते हैं
```

**नोड प्रकार (7):**

| प्रकार | किसके द्वारा बनाया गया | डिफ़ॉल्ट महत्व |
|------|-----------|-------------------|
| `decision` | मैन्युअल / MCP | 0.9 |
| `resolution` | मैन्युअल / MCP | 0.8 |
| `concept` | मैन्युअल / MCP | 0.7 |
| `project` | मैन्युअल / MCP | 0.7 |
| `pattern` | Auto (reflect) | 0.5 |
| `error` | Auto (reflect) | 0.4 |
| `session` | Auto (reflect) | 0.2 |

**मेमोरी जीवनचक्र:**

| घटना | क्या होता है |
|-------|-------------|
| search/recall/context के माध्यम से नोड रिकॉल | `access_count++`, `accessed_at` अपडेट |
| 30+ दिन बिना एक्सेस | महत्व 10% क्षय (न्यूनतम 0.05) |
| 180+ दिन बिना एक्सेस | `stale` टैग, रिकॉल से बाहर |
| `pinned` टैग वाला नोड | क्षय से प्रतिरक्षित |

**स्वचालित संचय की शर्तें:**

| शर्त | बनाया गया नोड |
|-----------|-------------|
| प्रत्येक सेशन समाप्ति | `session` (हमेशा) |
| समान त्रुटि लगातार ≥3 बार | `error` (repeated_same_error) |
| Edit→Error बारी-बारी | `pattern` (thrashing) |
| टूल सफलता दर <60% (न्यूनतम 5 अवलोकन) | `pattern` (weak_tool) |
| फ़ाइल प्रकार सफलता दर <50% (न्यूनतम 3 अवलोकन) | `pattern` (weak_filetype) |
| Edit सफलता → Bash त्रुटि चक्र | `pattern` (fix_then_break) |

> **नोट:** स्वच्छ सेशन (कोई त्रुटि नहीं) केवल `session` नोड उत्पन्न करते हैं। ग्राफ़ 2–3 वास्तविक विकास सेशन के बाद समृद्ध होता है जिनमें बिल्ड विफलताएँ, टेस्ट विफलताएँ, या डिबगिंग चक्र हों।

मौजूदा फ़ाइल-आधारित मेमोरी (`nodes/*.md`, `edges.jsonl`) पहले रन पर स्वचालित रूप से SQLite में माइग्रेट हो जाती हैं।

## कमांड

| कमांड | यह क्या करता है |
|---------|-------------|
| `/spec` | क्या बनाना है परिभाषित करें — आवश्यकताएँ स्पष्ट करें, एक स्पेक तैयार करें |
| `/go` | बनाएँ — ऑटो-प्लान, TDD सबएजेंट्स, समानांतर निष्पादन |
| `/check` | सत्यापित करें — समानांतर कोड रिव्यू + सुरक्षा ऑडिट + प्रदर्शन |
| `/ship` | शिप करें — PR, CI, मर्ज |
| `/team` | प्रोजेक्ट्स में संगठन-स्तरीय एजेंट टीम बनाएँ और सिंक करें |
| `/evolve` | मैन्युअल इवोल्यूशन ट्रिगर / स्थिति / रोलबैक |

## टीम (`epic team`)

टीम **org-level** हैं, प्रोजेक्ट-बाउंड नहीं। किसी भी प्रोजेक्ट में `/team` चलाने से साझा एजेंट परिभाषाओं का पूल समृद्ध होता है — कभी चुपचाप ओवरराइट नहीं करता।

### यह कैसे काम करता है

```
epic team                      # इंटरएक्टिव: प्रोजेक्ट स्कैन → डिज़ाइन → लिखें → सिंक
         ↓
~/.harness/orgs/epic/teams/backend/   ← ग्लोबल स्टोर (प्रोजेक्ट्स में बना रहता है)
         ↓
epic team sync backend
         ↓
{project}/.claude/agents/backend/     ← Claude Code सेशन स्टार्ट पर ऑटो-डिस्कवर करता है
├── domain-expert.md                  ← भूमिका परिभाषा + टीम कॉन्टेक्स्ट इंजेक्ट
├── reviewer.md
└── tester.md
         ↓
अगला सेशन: एजेंट सक्रिय — Claude द्वारा ऑटो-चुने या स्पष्ट रूप से कॉल
```

### CLI संदर्भ

```bash
# टीम बनाएँ या अपडेट करें (इंटरएक्टिव 4-फ़ेज़ फ़्लो)
epic team

# ब्राउज़ करें
epic team list                        # वर्तमान org की सभी टीम
epic team list --org netflix          # नामित org की टीम
epic team show backend                # कॉन्फ़िग, मिशन, एजेंट
epic team show backend --playbook     # + पूरा संचित प्लेबुक

# प्रोजेक्ट में डिस्पैच करें
epic team sync backend                # डिस्पैच: एजेंट कॉपी करें → .claude/agents/backend/
epic team link backend                # डिस्पैच + टीम कॉन्फ़िग में प्रोजेक्ट रजिस्टर

# प्रोजेक्ट से रिकॉल करें
epic team delete backend              # रिकॉल: केवल वर्तमान प्रोजेक्ट से हटाएँ
epic team unlink backend              # delete का उपनाम

# भंग करें (org से पूरी तरह हटाएँ)
epic team delete backend --global     # org स्टोर + लोकल कॉपी से स्थायी रूप से हटाएँ

# इतिहास
epic team history backend reviewer    # एजेंट के .history/ बैकअप सूची
```

### कोडिंग एजेंट्स से टीम का उपयोग

सिंक के बाद, एजेंट अगले सेशन में स्वचालित रूप से उपलब्ध हैं:

```
# Claude Code / Cursor / OpenCode / Codex
@domain-expert पेमेंट गेटवे इम्प्लीमेंट करें
@reviewer इस PR में एज केसेज़ चेक करें
@tester auth के लिए इंटीग्रेशन टेस्ट लिखें

# या एजेंट को टास्क कॉन्टेक्स्ट के आधार पर ऑटो-चुनने दें
```

प्रत्येक एजेंट फ़ाइल में सिंक समय पर इंजेक्ट किया गया **टीम कॉन्टेक्स्ट** सेक्शन होता है:

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end
**Full playbook**: `epic team show backend --playbook`
```

एजेंट अपनी टीम, मिशन और आवश्यकतानुसार पूरा प्लेबुक लोड करने का तरीका जानते हैं —
कॉन्टेक्स्ट विंडो को इससे बोझिल किए बिना।

### मल्टी-org

```bash
epic team                          # "epic" org में संचित (डिफ़ॉल्ट)
epic team --org netflix            # अलग Netflix-शैली टोपोलॉजी
epic team --org client-x           # प्रति-क्लाइंट एंगेजमेंट
```

एक ही org में एक ही टीम नाम = जानबूझकर क्रॉस-प्रोजेक्ट शेयरिंग।
`epic/teams/backend` हर प्रोजेक्ट से ज्ञान संचित करता है जो इसे बनाता या लिंक करता है।

### टीम प्रकार

| प्रकार | कीवर्ड | डिफ़ॉल्ट एजेंट |
|------|---------|---------------|
| Stream-aligned | `stream` | domain-expert, reviewer, tester |
| Platform | `platform` | api-designer, infra-specialist, dx-agent |
| Enabling | `enabling` | specialist |
| Complicated Subsystem | `subsystem` | domain-specialist, integration-tester |

### मर्ज रणनीति — कोई चुप्पी से ओवरराइट नहीं

| ऑब्जेक्ट | नियम |
|--------|------|
| एजेंट — नया | ऑटो-जोड़ें |
| एजेंट — अपरिवर्तित | स्किप |
| एजेंट — बदला | **प्रॉम्प्ट** (डिफ़ॉल्ट: मौजूदा रखें)। बदलने पर → `.history/` में बैकअप |
| `playbook.md` | हमेशा **अपेंड** — कभी ट्रंकेट नहीं |
| `mission.md` — बदला | **प्रॉम्प्ट** (डिफ़ॉल्ट: मौजूदा रखें) |

## ऑटो स्किल्स (Ring 2)

स्किल्स संदर्भ के आधार पर स्वचालित रूप से ट्रिगर होती हैं। आपको इन्हें इनवोक करने की आवश्यकता नहीं है।

| स्किल | कब ट्रिगर होती है |
|-------|--------------|
| **tdd** | नई फ़ीचर इम्प्लीमेंटेशन |
| **debug** | टेस्ट विफलता या एरर |
| **secure** | Auth/DB/API/secrets कोड को छुआ गया |
| **perf** | लूप्स, क्वेरीज़, रेंडरिंग कोड |
| **simplify** | फ़ाइल > 200 लाइन या उच्च जटिलता |
| **document** | पब्लिक API जोड़ा या बदला गया |
| **verify** | /go या /ship पूरा करने से पहले |
| **context** | कॉन्टेक्स्ट विंडो > 70% उपयोग |

## हुक्स (Ring 0)

अदृश्य रूप से चलते हैं। किसी उपयोगकर्ता कार्रवाई की आवश्यकता नहीं। एक **सिंगल Rust बाइनरी** (`epic-harness`) के रूप में सबकमांड्स के साथ लागू किए गए, बाइनरी अनुपस्थित होने पर Node.js पर फ़ॉलबैक।

```
epic resume | guard | polish | observe | snapshot | reflect
```

| हुक | कब | क्या करता है |
|------|------|------|
| **resume** | सेशन शुरू | कॉन्टेक्स्ट रिस्टोर, मेमोरी लोड, स्टैक डिटेक्ट |
| **guard** | Bash से पहले | force-push-to-main, rm -rf /, DROP prod ब्लॉक करें |
| **polish** | Edit के बाद | ऑटो-फॉर्मेट (Biome/Prettier/ruff/gofmt) + टाइपचेक |
| **observe** | हर टूल उपयोग | इवोल्यूशन के लिए `~/.harness/projects/{slug}/obs/` में लॉग |
| **snapshot** | कॉम्पैक्ट से पहले | `~/.harness/projects/{slug}/sessions/` में स्थिति सेव |
| **reflect** | सेशन समाप्त | विफलताओं का विश्लेषण, इवॉल्व्ड स्किल्स सीड, गेट |

## इवैल सिस्टम (Ring 3 कोर)

A-Evolve के बेंचमार्क पैटर्न को Claude Code के हुक सिस्टम में फ़्यूज़ करता है।

### बहु-आयामी स्कोरिंग

प्रत्येक टूल कॉल को 3 अक्षों पर स्कोर किया जाता है। वेट `src/hooks/common.rs` में `SCORE_WEIGHTS` के माध्यम से कॉन्फ़िगर करने योग्य हैं:

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (default: 0.5)                          (default: 0.3)                             (default: 0.2)
```

| आयाम | क्या मापता है | प्रति-टूल मानदंड |
|-----------|-----------------|-------------------|
| `tool_success` | क्या यह काम किया? (0/1) | 9-श्रेणी विफलता वर्गीकरण |
| `output_quality` | आउटपुट गुणवत्ता संकेत (0.0-1.0) | Bash: चेतावनियाँ, खाली आउटपुट। Edit: री-एडिट डिटेक्शन |
| `execution_cost` | दक्षता प्रॉक्सी (0.0-1.0) | आउटपुट साइज़, साइलेंट-सक्सेस कमांड व्हाइटलिस्ट |

### विफलता वर्गीकरण (9 श्रेणियाँ)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### पैटर्न डिटेक्शन (4 प्रकार)

सभी थ्रेशोल्ड `src/hooks/common.rs` में कॉन्फ़िगर करने योग्य कॉन्स्टेंट्स हैं:

| पैटर्न | क्या पहचानता है | कॉन्स्टेंट | डिफ़ॉल्ट |
|---------|---------|----------|---------|
| `repeated_same_error` | एक ही एरर N+ बार लगातार | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edit सफल → build/test विफल | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | एक ही फ़ाइल पर N+ ऑपरेशन अटका | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | एक ही फ़ाइल पर Edit↔Error बारी-बारी | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### स्किल सीडिंग थ्रेशोल्ड

| ट्रिगर | कॉन्स्टेंट | डिफ़ॉल्ट |
|---------|----------|---------|
| कमज़ोर टूल (कम सफलता दर) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| कमज़ोर फ़ाइल प्रकार | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| उच्च-आवृत्ति एरर | `HIGH_FREQ_ERROR_MIN` | 5 |

### स्टैग्नेशन गेटिंग

- `STAGNATION_LIMIT` (डिफ़ॉल्ट: 3) सेशन बिना सुधार के → इवॉल्व्ड स्किल्स का सर्वोत्तम चेकपॉइंट पर ऑटो-रोलबैक
- `IMPROVEMENT_THRESHOLD` (डिफ़ॉल्ट: 5%)
- ट्रेंड ट्रैकिंग: लीनियर रिग्रेशन के माध्यम से `improving` / `stable` / `declining`
- टकराव पर स्टैटिक स्किल्स हमेशा इवॉल्व्ड स्किल्स पर प्राथमिकता लेती हैं

### इवोल्यूशन फ़्लो

```
Observe (PostToolUse — 3-अक्ष स्कोरिंग)
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: प्रति-टूल, प्रति-ext, स्कोर वितरण
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 पथ: pattern / weak tool / weak file type / high-freq error)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate (फॉर्मेट चेक, डीडुप, 10 की सीमा, स्टैग्नेशन चेक)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (सर्वोत्तम चेकपॉइंट)
Reload (अगला सेशन — resume.ts मेट्रिक्स रिपोर्ट + इवॉल्व्ड स्किल्स लोड करता है)
```

```bash
/evolve              # अभी इवोल्यूशन चलाएँ
/evolve status       # डैशबोर्ड: स्कोर, ट्रेंड, पैटर्न, स्किल्स
/evolve history      # दीर्घकालिक विश्लेषण: पूर्ण इतिहास, स्किल प्रभावशीलता, डिस्पैच आँकड़े
/evolve cross-project # क्रॉस-प्रोजेक्ट पैटर्न विश्लेषण
/evolve rollback     # पिछला सर्वोत्तम पुनर्स्थापित करें
/evolve reset        # सभी इवोल्यूशन डेटा साफ़ करें
```

## कोल्ड-स्टार्ट प्रीसेट्स

उपयोगी इवॉल्व्ड स्किल्स के लिए 5 सेशन इंतज़ार करने की ज़रूरत नहीं। पहले सेशन पर, epic harness आपके स्टैक का पता लगाता है और प्रीसेट स्किल्स स्वचालित रूप से लागू करता है:

| स्टैक | प्रीसेट स्किल्स |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

प्रीसेट्स पूरक हैं — डेटा जमा होने पर वे वास्तविक इवॉल्व्ड स्किल्स से बदल दिए जाते हैं।

## समवर्ती सेशन सुरक्षा

प्रत्येक सेशन अपनी ऑब्ज़र्वेशन फ़ाइल (`session_{date}_{pid}_{random}.jsonl`) में लिखता है। एक ही प्रोजेक्ट पर कई Claude Code सेशन एक-दूसरे के डेटा को दूषित नहीं करेंगे। reflect हुक विश्लेषण के लिए एक ही दिन की सभी फ़ाइलों को मर्ज करता है।

## कस्टम गार्ड नियम

प्रोजेक्ट रूट में `.harness/guard-rules.yaml` के माध्यम से प्रोजेक्ट-विशिष्ट सुरक्षा नियम जोड़ें:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

नियम अंतर्निहित गार्ड्स (force-push-to-main, rm -rf /, DROP prod) के साथ मर्ज होते हैं। इस फ़ाइल को git में रखने से अपनी टीम के साथ सुरक्षा नियम साझा करने में मदद मिलती है।

## क्रॉस-प्रोजेक्ट लर्निंग

प्रोजेक्ट्स के बीच विफलता पैटर्न साझा करने के लिए ऑप्ट-इन करें:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # ऑप्ट-इन
```

सक्षम होने पर:
- सेशन समाप्त होने पर गुमनाम पैटर्न `~/.harness/global_patterns.jsonl` में निर्यात होते हैं
- सेशन शुरू होने पर अन्य प्रोजेक्ट्स के कमज़ोर क्षेत्रों से संकेत दिखाए जाते हैं
- समग्र पैटर्न देखने के लिए `/evolve cross-project` का उपयोग करें

## स्किल प्रभावशीलता ट्रैकिंग

प्रत्येक इवॉल्व्ड स्किल को A/B एट्रिब्यूशन स्कोर के साथ ट्रैक किया जाता है:

```
/evolve history → Skill Effectiveness सेक्शन

| Skill              | Sessions | Score With | Score Without | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

सकारात्मक डेल्टा = स्किल मदद करती है। नकारात्मक डेल्टा = `/evolve rollback` के माध्यम से हटाने पर विचार करें।

## Polish → Observe फ़ीडबैक

polish हुक (ऑटो-फॉर्मेट + टाइपचेक) परिणामों को ऑब्ज़र्वेशन पाइपलाइन में वापस फ़ीड करता है:

- फ़ॉर्मेट विफलता → `lint_fail` के रूप में रिकॉर्ड
- TypeScript एरर → `build_fail` के रूप में रिकॉर्ड
- सफलताएँ → पूर्ण स्कोर के साथ रिकॉर्ड

इसका मतलब है कि "edit → type error → edit → type error" थ्रैशिंग पैटर्न का पता तब भी लगाया जाता है जब एरर मैन्युअल कमांड से नहीं बल्कि polish हुक से आते हैं।

## प्रोजेक्ट डेटा (`~/.harness/projects/{slug}/`)

प्रोजेक्ट-विशिष्ट डेटा आपके होम डायरेक्टरी में रहता है। यह प्रोजेक्ट डिलीशन के बाद भी बना रहता है और आपकी git हिस्ट्री को दूषित नहीं करता।

```
~/.harness/projects/{slug}/
├── memory/           # प्रोजेक्ट पैटर्न और नियम (स्थायी)
├── sessions/         # सेशन स्नैपशॉट (resume के लिए)
├── obs/              # टूल उपयोग ऑब्ज़र्वेशन लॉग (JSONL, प्रति-सेशन)
├── evolved/          # ऑटो-इवॉल्व्ड स्किल्स
├── evolved_backup/   # सर्वोत्तम चेकपॉइंट (स्टैग्नेशन रोलबैक के लिए)
├── dispatch/         # स्किल डिस्पैच लॉग (JSONL)
├── team/             # legacy (superseded by ~/.harness/orgs/)
├── evolution.jsonl   # पूर्ण इवोल्यूशन इतिहास
└── metrics.json      # समग्र आँकड़े + स्किल एट्रिब्यूशन

~/.harness/
├── memory.db         # SQLite नॉलेज ग्राफ़ (नोड्स + एज + FTS5)
├── graph.json        # कैश्ड ग्राफ़ (वेब UI के लिए)
└── orgs/             # epic team ग्लोबल स्टोर
    └── {org}/
        └── teams/
            └── {team}/
                ├── config.json
                ├── mission.md
                ├── playbook.md
                ├── agents/
                └── .history/
```

आप अपनी टीम के साथ सुरक्षा नियम साझा करने के लिए प्रोजेक्ट रूट में `.harness/guard-rules.yaml` का उपयोग जारी रख सकते हैं।

## डेवलपमेंट

### बिल्ड

```bash
cargo install --path .          # बिल्ड + ~/.cargo/bin/ में इंस्टॉल
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # प्लगइन बाइनरी अपडेट
```

### हुक्स कैसे डिस्पैच होते हैं

`hooks.json` में प्रत्येक हुक दो स्थानों पर Rust बाइनरी खोजता है:

```
1. प्लगइन लोकल: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (cargo install के माध्यम से)
```

### टेस्ट

```bash
cargo test       # Rust यूनिट + इंटीग्रेशन टेस्ट
```

## आभार

epic harness निम्नलिखित प्रोजेक्ट्स के विचारों से प्रेरित और उन पर निर्मित है:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — स्वचालित इवोल्यूशन और बेंचमार्क पैटर्न
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code एजेंट स्किल सिस्टम
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — व्यापक Claude Code पैटर्न
- [gstack](https://github.com/garrytan/gstack) — प्लगइन आर्किटेक्चर संदर्भ
- [harness](https://github.com/revfactory/harness) — हुक और हार्नेस इंफ्रास्ट्रक्चर पैटर्न
- [serena](https://github.com/oraios/serena) — स्वायत्त एजेंट डिज़ाइन
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — मल्टी-कमांड फ्रेमवर्क आर्किटेक्चर
- [superpowers](https://github.com/obra/superpowers) — Claude Code एक्सटेंशन पैटर्न

## लाइसेंस

[Apache 2.0](LICENSE)
