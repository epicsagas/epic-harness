PLUGIN_CACHE := $(HOME)/.claude/plugins/cache/epicsagas/epic/0.1.0/hooks/bin/epic-harness
CARGO_BIN    := $(HOME)/.cargo/bin/epic-harness
HOOKS_BIN    := hooks/bin/epic-harness

.PHONY: build install dashboard-build gen-skills gen-skills-dry lint-skills eval demo-record demo-gif demo-clean

# Build the Svelte dashboard and copy into assets/ (called automatically by build.rs during cargo build)
dashboard-build:
	cd app && pnpm install --frozen-lockfile && pnpm run build
	cp app/dist/index.html assets/dashboard.html
	@echo "dashboard built → assets/dashboard.html"

build: dashboard-build
	cargo build --release

install: build
	cp target/release/epic-harness $(HOOKS_BIN)
	cp target/release/epic-harness $(CARGO_BIN)
	@if [ -f "$(PLUGIN_CACHE)" ]; then cp target/release/epic-harness "$(PLUGIN_CACHE)"; fi
	@echo "installed: $(HOOKS_BIN), $(CARGO_BIN)"

# Validate the same skills directory published by the plugin manifest.
# These targets are read-only; CSO wording warnings remain advisory.
gen-skills:
	node scripts/lint-skills.mjs

gen-skills-dry:
	node scripts/lint-skills.mjs --dry-run

lint-skills:
	node scripts/lint-skills.mjs

# Run project quality & regression evaluation
eval:
	@echo "Running eval..."
	@epic-harness eval --json
	@echo "Eval complete."

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
