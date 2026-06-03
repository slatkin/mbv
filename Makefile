BINARY     := mby
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR    := $(HOME)/.local/share/mby
CONFIG_DIR  := $(HOME)/.config/mby

.PHONY: all build install clean

all: build

build:
	cargo build --release

install: build
	install -Dm755 target/release/$(BINARY) $(INSTALL_DIR)/$(BINARY)
	install -Dm644 scripts/mby.lua $(DATA_DIR)/scripts/mby.lua
	install -Dm644 fonts/Material-Design-Iconic-Font.ttf $(DATA_DIR)/fonts/Material-Design-Iconic-Font.ttf
	@if [ ! -f $(CONFIG_DIR)/config.toml ]; then \
		install -Dm644 dist/config.toml $(CONFIG_DIR)/config.toml; \
		echo "Installed default config to $(CONFIG_DIR)/config.toml"; \
	else \
		echo "Config already exists at $(CONFIG_DIR)/config.toml — skipping"; \
	fi

clean:
	cargo clean
