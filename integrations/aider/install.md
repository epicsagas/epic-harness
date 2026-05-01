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

## Verify
```bash
aider --help | grep -i config
cat .aider.conf.yml
```
