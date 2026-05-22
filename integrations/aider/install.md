# Aider Integration — epic-harness

## Install

### Project-level (recommended)
```bash
cp .aider.conf.yml <your-project>/.aider.conf.yml
cp -r .aider <your-project>/.aider
```

### Global
```bash
mkdir -p ~/.aider
cp .aider/CONVENTIONS.md ~/.aider/CONVENTIONS.md
```

## What You Get
- **CONVENTIONS.md**: Coding standards (TDD, security, quality gates)
- **.aider.conf.yml**: Auto-loads conventions in every session

## Orbit — Autonomous Pipeline

When the user says "orbit", the full spec→ship pipeline runs automatically:

1. **Mode selection**: Ask user — interactive (user describes the problem, harness frames it) or council auto-spec (4-voice generates spec, user approves)
2. **After spec approved**: build → test → review → PR automatically (sequential, aider is single-threaded)
3. **On review failure**: fix and re-test, max 3 retries then pause for user
4. **Produce consolidated report** at end

State tracked in `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`.

## Verify
```bash
aider --help | grep -i config
cat .aider.conf.yml
```
