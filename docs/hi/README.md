# epic harness

**6 कमांड। ऑटो-ट्रिगर स्किल्स। स्वयं-विकसित होने वाला।**

<p align="center">
<a href="../../README.md">English</a> | <a href="../ja/README.md">日本語</a> | <a href="../ko/README.md">한국어</a> | <a href="../de/README.md">Deutsch</a> | <a href="../fr/README.md">Français</a> | <a href="../zh-CN/README.md">简体中文</a> | <a href="../zh-TW/README.md">繁體中文</a> | <a href="../pt-BR/README.md">Português</a> | <a href="../es/README.md">Español</a> | <a href="../hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="https://github.com/epicsagas/epic-harness/actions/workflows/ci.yml"><img src="https://github.com/epicsagas/epic-harness/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
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

```bash
# Claude Code प्लगइन मार्केटप्लेस
/plugin marketplace add epicsagas/epic-harness
/plugin install harness@epic

# या मैन्युअल रूप से
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rust बाइनरी (वैकल्पिक, ~4x तेज़ हुक्स)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness


# crates.io से
cargo install epic-harness
# या cargo-binstall से (पूर्व-निर्मित, तेज़)
cargo binstall epic-harness

# सोर्स से
cargo install --path .
```

बाइनरी हुक्स द्वारा स्वचालित रूप से पहचानी जाती है। अनुपस्थित होने पर, हुक्स Node.js पर फ़ॉलबैक करते हैं।

## कमांड

| कमांड | यह क्या करता है |
|---------|-------------|
| `/spec` | क्या बनाना है परिभाषित करें — आवश्यकताएँ स्पष्ट करें, एक स्पेक तैयार करें |
| `/go` | बनाएँ — ऑटो-प्लान, TDD सबएजेंट्स, समानांतर निष्पादन |
| `/check` | सत्यापित करें — समानांतर कोड रिव्यू + सुरक्षा ऑडिट + प्रदर्शन |
| `/ship` | शिप करें — PR, CI, मर्ज |
| `/team` | प्रोजेक्ट-विशिष्ट एजेंट टीम डिज़ाइन करें |
| `/evolve` | मैन्युअल इवोल्यूशन ट्रिगर / स्थिति / रोलबैक |

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

अदृश्य रूप से चलते हैं। किसी उपयोगकर्ता कार्रवाई की आवश्यकता नहीं। एक **सिंगल Rust बाइनरी** (`epic-harness`) के रूप में सबकमांड्स के साथ लागू किए गए, बाइनरी उपलब्ध न होने पर Node.js पर फ़ॉलबैक करते हैं।

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| हुक | कब | क्या करता है |
|------|------|------|
| **resume** | सेशन शुरू | कॉन्टेक्स्ट रिस्टोर, मेमोरी लोड, स्टैक डिटेक्ट |
| **guard** | Bash से पहले | force-push-to-main, rm -rf /, DROP prod ब्लॉक करें |
| **polish** | Edit के बाद | ऑटो-फॉर्मेट (Biome/Prettier/ruff/gofmt) + टाइपचेक |
| **observe** | हर टूल उपयोग | इवोल्यूशन के लिए `.harness/obs/` में लॉग |
| **snapshot** | कॉम्पैक्ट से पहले | `.harness/sessions/` में स्थिति सेव |
| **reflect** | सेशन समाप्त | विफलताओं का विश्लेषण, इवॉल्व्ड स्किल्स सीड, गेट |

## इवैल सिस्टम (Ring 3 कोर)

A-Evolve के बेंचमार्क पैटर्न को Claude Code के हुक सिस्टम में फ़्यूज़ करता है।

### बहु-आयामी स्कोरिंग

प्रत्येक टूल कॉल को 3 अक्षों पर स्कोर किया जाता है। वेट `src/ts/common.ts` (या `src/hooks/common.rs`) में `SCORE_WEIGHTS` के माध्यम से कॉन्फ़िगर करने योग्य हैं:

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

सभी थ्रेशोल्ड `src/ts/common.ts` (या `src/hooks/common.rs`) में कॉन्फ़िगर करने योग्य कॉन्स्टेंट्स हैं:

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
    ↓ .harness/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: प्रति-टूल, प्रति-ext, स्कोर वितरण
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 पथ: pattern / weak tool / weak file type / high-freq error)
    ↓ .harness/evolved/{skill}/SKILL.md
Gate (फॉर्मेट चेक, डीडुप, 10 की सीमा, स्टैग्नेशन चेक)
    ↓ .harness/evolved_backup/ (सर्वोत्तम चेकपॉइंट)
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

`.harness/guard-rules.yaml` के माध्यम से प्रोजेक्ट-विशिष्ट सुरक्षा नियम जोड़ें:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

नियम अंतर्निहित गार्ड्स (force-push-to-main, rm -rf /, DROP prod) के साथ मर्ज होते हैं।

## क्रॉस-प्रोजेक्ट लर्निंग

प्रोजेक्ट्स के बीच विफलता पैटर्न साझा करने के लिए ऑप्ट-इन करें:

```bash
touch .harness/.cross-project-enabled  # ऑप्ट-इन
```

सक्षम होने पर:
- सेशन समाप्त होने पर गुमनाम पैटर्न `~/.harness-global/patterns.jsonl` में निर्यात होते हैं
- सेशन शुरू होने पर अन्य प्रोजेक्ट्स के कमज़ोर क्षेत्रों से संकेत दिखाए जाते हैं
- समग्र पैटर्न देखने के लिए `/evolve cross-project` का उपयोग करें

## स्किल प्रभावशीलता ट्रैकिंग

प्रत्येक इवॉल्व्ड स्किल को A/B एट्रिब्यूशन स्कोर के साथ ट्रैक किया जाता है:

```
/evolve history → Skill Effectiveness section

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

## प्रोजेक्ट डेटा (`.harness/`)

epic harness आपके प्रोजेक्ट में एक `.harness/` डायरेक्टरी बनाता है:

```
.harness/
├── memory/           # प्रोजेक्ट पैटर्न और नियम (स्थायी)
├── sessions/         # सेशन स्नैपशॉट (resume के लिए)
├── obs/              # टूल उपयोग ऑब्ज़र्वेशन लॉग (JSONL, प्रति-सेशन)
├── evolved/          # ऑटो-इवॉल्व्ड स्किल्स
├── evolved_backup/   # सर्वोत्तम चेकपॉइंट (स्टैग्नेशन रोलबैक के लिए)
├── dispatch/         # स्किल डिस्पैच लॉग (JSONL)
├── team/             # /team द्वारा उत्पन्न एजेंट्स और स्किल्स
├── evolution.jsonl   # पूर्ण इवोल्यूशन इतिहास
├── metrics.json      # समग्र आँकड़े + स्किल एट्रिब्यूशन
└── guard-rules.yaml  # कस्टम गार्ड नियम (वैकल्पिक)
```

`.harness/` को `.gitignore` में जोड़ें या कमिट करें — आपकी पसंद।

## डेवलपमेंट

### Rust (प्राथमिक — ~4x तेज़)

```bash
cargo install --path .          # बिल्ड + ~/.cargo/bin/ में इंस्टॉल
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # प्लगइन बाइनरी अपडेट
```

### Node.js (फ़ॉलबैक)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### हुक्स कैसे डिस्पैच होते हैं

`hooks.json` में प्रत्येक हुक तीन स्थानों पर Rust बाइनरी खोजता है, फिर Node.js पर फ़ॉलबैक करता है:

```
1. प्लगइन लोकल: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (cargo install के माध्यम से)
3. फ़ॉलबैक:     node hooks/scripts/<hook>.js
```

### टेस्ट

```bash
cargo test       # 98 Rust यूनिट टेस्ट
npm test         # Node.js यूनिट + e2e टेस्ट
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
