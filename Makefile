PLUGIN_CACHE := $(HOME)/.claude/plugins/cache/epicsagas/epic/0.1.0/hooks/bin/epic-harness
CARGO_BIN    := $(HOME)/.cargo/bin/epic-harness
HOOKS_BIN    := hooks/bin/epic-harness

.PHONY: build install dashboard-build gen-skills gen-skills-dry lint-skills demo-record demo-gif demo-clean

# Build the Svelte dashboard and copy into assets/ (called automatically by build.rs during cargo build)
dashboard-build:
	cd app && npm install --prefer-offline && npm run build
	cp app/dist/index.html assets/dashboard.html
	@echo "dashboard built → assets/dashboard.html"

build: dashboard-build
	cargo build --release

install: build
	cp target/release/epic-harness $(HOOKS_BIN)
	cp target/release/epic-harness $(CARGO_BIN)
	@if [ -f "$(PLUGIN_CACHE)" ]; then cp target/release/epic-harness "$(PLUGIN_CACHE)"; fi
	@echo "installed: $(HOOKS_BIN), $(CARGO_BIN)"

# Generate/validate SKILL.md files across all skills
gen-skills:
	@echo "Validating skills..."
	@missing=0; \
	for dir in registry/skills/*/; do \
		skill_file="$$dir/SKILL.md"; \
		if [ ! -f "$$skill_file" ]; then \
			echo "MISSING: $$skill_file"; \
			missing=$$((missing + 1)); \
		elif ! head -1 "$$skill_file" | grep -q '^---'; \
		then \
			echo "MALFORMED (no frontmatter): $$skill_file"; \
			missing=$$((missing + 1)); \
		elif ! grep -q '^name:' "$$skill_file"; \
		then \
			echo "MALFORMED (no name field): $$skill_file"; \
			missing=$$((missing + 1)); \
		elif ! grep -q '^description:' "$$skill_file"; \
		then \
			echo "MALFORMED (no description field): $$skill_file"; \
			missing=$$((missing + 1)); \
		else \
			name=$$(grep '^name:' "$$skill_file" | head -1 | sed 's/name: *//' | tr -d '"'); \
			echo "OK: $$name ($$skill_file)"; \
		fi; \
	done; \
	if [ $$missing -gt 0 ]; then \
		echo "FAIL: $$missing skill(s) need attention"; \
		exit 1; \
	else \
		echo "All skills validated successfully"; \
	fi

# Dry-run mode: shows what would change (for CI stagnation detection)
gen-skills-dry:
	@echo "Dry-run skill validation..."
	@for dir in registry/skills/*/; do \
		skill_file="$$dir/SKILL.md"; \
		if [ ! -f "$$skill_file" ]; then \
			echo "WOULD CREATE: $$skill_file"; \
		else \
			echo "EXISTS: $$skill_file"; \
		fi; \
	done
	@echo "Dry-run complete. No files modified."

# Lint skills for CSO compliance (description should be trigger conditions only)
lint-skills:
	@echo "Linting skill descriptions..."
	@issues=0; \
	for dir in registry/skills/*/; do \
		skill_file="$$dir/SKILL.md"; \
		if [ -f "$$skill_file" ]; then \
			if grep -qiE 'workflow|step [0-9]|phase [0-9]|dispatch' "$$skill_file" 2>/dev/null; then \
				echo "CSO WARNING: $$skill_file may contain workflow summary in frontmatter"; \
				issues=$$((issues + 1)); \
			fi; \
		fi; \
	done; \
	if [ $$issues -gt 0 ]; then \
		echo "Found $$issues issue(s). Descriptions should contain trigger conditions only."; \
	else \
		echo "All skill descriptions pass CSO lint."; \
	fi

# ── Demo recording (episteme + orbit) ────────────────────────────
DEMO_DIR  := docs/demo
DEMO_CAST := $(DEMO_DIR)/episteme-orbit.cast
DEMO_GIF  := $(DEMO_DIR)/episteme-orbit.gif
DEMO_ROWS := 40
DEMO_COLS := 120
DEMO_FPS  := 12

demo-prepare:
	@zsh $(DEMO_DIR)/prepare.sh

demo-record:
	@zsh $(DEMO_DIR)/prepare.sh --record

demo-gif:
	@if [ ! -f $(DEMO_CAST) ]; then echo "Run 'make demo-record' first"; exit 1; fi
	@echo "Converting $(DEMO_CAST) → $(DEMO_GIF) ($(DEMO_FPS)fps)..."
	@agg --rows $(DEMO_ROWS) --cols $(DEMO_COLS) --fps $(DEMO_FPS) $(DEMO_CAST) $(DEMO_GIF)
	@ls -lh $(DEMO_GIF)

demo-clean:
	@rm -f $(DEMO_CAST) $(DEMO_GIF)
	@rm -rf examples/user-api-demo
	@echo "Cleaned demo artifacts."
