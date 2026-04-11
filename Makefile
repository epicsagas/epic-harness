PLUGIN_CACHE := $(HOME)/.claude/plugins/cache/epicsagas/epic/0.1.0/hooks/bin/epic-harness
CARGO_BIN    := $(HOME)/.cargo/bin/epic-harness
HOOKS_BIN    := hooks/bin/epic-harness

.PHONY: build install

build:
	cargo build --release

install: build
	cp target/release/epic-harness $(HOOKS_BIN)
	cp target/release/epic-harness $(CARGO_BIN)
	@if [ -f "$(PLUGIN_CACHE)" ]; then cp target/release/epic-harness "$(PLUGIN_CACHE)"; fi
	@echo "installed: $(HOOKS_BIN), $(CARGO_BIN)"
