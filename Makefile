SHELL := /usr/bin/env bash

NODE_VERSION ?= v22.22.3
NODE_ARCHIVE := node-$(NODE_VERSION)-linux-x64.tar.xz
NODE_ROOT ?= /var/tmp/deepseek-mobile-web-node
NODE_DIR := $(NODE_ROOT)/node-$(NODE_VERSION)-linux-x64
NODE_BIN := $(NODE_DIR)/bin/node
NPM_BIN := $(NODE_DIR)/bin/npm

HOST ?= 0.0.0.0
PORT ?= 8788
SSH_HOST ?= 192.168.30.244
SSH_USER ?= root
SSH_PORT ?= 22
STATIC_DIR ?= mobile-web/dist
ACCESS_TOKEN ?=

.PHONY: help mobile-web-node mobile-web-install mobile-web-build mobile-web-server-build mobile-web-server mobile-web-smoke

help:
	@echo "Targets:"
	@echo "  make mobile-web-build   Prepare local Node, install deps, build mobile-web/dist"
	@echo "  make mobile-web-server  Build frontend and Rust server, then listen on $(HOST):$(PORT)"
	@echo "  make mobile-web-smoke   Run smoke scripts against http://127.0.0.1:$(PORT)"
	@echo ""
	@echo "Variables:"
	@echo "  HOST=$(HOST) PORT=$(PORT) SSH_HOST=$(SSH_HOST) SSH_USER=$(SSH_USER) SSH_PORT=$(SSH_PORT)"
	@echo "  ACCESS_TOKEN=<optional>"

mobile-web-node:
	@if [[ -x "$(NODE_BIN)" && -x "$(NPM_BIN)" ]]; then \
		echo "Using local Node: $(NODE_BIN)"; \
	else \
		echo "Installing local Node $(NODE_VERSION) under $(NODE_ROOT)"; \
		mkdir -p "$(NODE_ROOT)"; \
		curl -fsSL "https://nodejs.org/dist/$(NODE_VERSION)/$(NODE_ARCHIVE)" -o "$(NODE_ROOT)/$(NODE_ARCHIVE)"; \
		tar -xJf "$(NODE_ROOT)/$(NODE_ARCHIVE)" -C "$(NODE_ROOT)"; \
	fi
	@"$(NODE_BIN)" --version
	@PATH="$(NODE_DIR)/bin:$$PATH" "$(NPM_BIN)" --version

mobile-web-install: mobile-web-node
	cd mobile-web && PATH="$(NODE_DIR)/bin:$$PATH" npm install

mobile-web-build: mobile-web-install
	cd mobile-web && PATH="$(NODE_DIR)/bin:$$PATH" npm run typecheck
	cd mobile-web && PATH="$(NODE_DIR)/bin:$$PATH" npm run build

mobile-web-server-build:
	cargo build -p deepseek-mobile-web-server

mobile-web-server: mobile-web-build mobile-web-server-build
	@echo "Open from this machine: http://127.0.0.1:$(PORT)"
	@echo "Open from phone:       http://<LAN-IP>:$(PORT)"
	@echo "LAN IP candidates:     $$(hostname -I 2>/dev/null || true)"
	@if [[ -n "$(ACCESS_TOKEN)" ]]; then \
		echo "Access token: enabled"; \
		target/debug/deepseek-mobile-web-server \
			--host "$(HOST)" \
			--port "$(PORT)" \
			--ssh-host "$(SSH_HOST)" \
			--ssh-user "$(SSH_USER)" \
			--ssh-port "$(SSH_PORT)" \
			--static-dir "$(STATIC_DIR)" \
			--access-token "$(ACCESS_TOKEN)"; \
	else \
		echo "Access token: disabled"; \
		target/debug/deepseek-mobile-web-server \
			--host "$(HOST)" \
			--port "$(PORT)" \
			--ssh-host "$(SSH_HOST)" \
			--ssh-user "$(SSH_USER)" \
			--ssh-port "$(SSH_PORT)" \
			--static-dir "$(STATIC_DIR)"; \
	fi

mobile-web-smoke:
	@if [[ -n "$(ACCESS_TOKEN)" ]]; then \
		python3 scripts/mobile_web_ssh_smoke.py --server "http://127.0.0.1:$(PORT)" --access-token "$(ACCESS_TOKEN)" --json; \
		python3 scripts/mobile_web_ssh_flow_simulator.py --server "http://127.0.0.1:$(PORT)" --access-token "$(ACCESS_TOKEN)" --auto-approve --json; \
	else \
		python3 scripts/mobile_web_ssh_smoke.py --server "http://127.0.0.1:$(PORT)" --json; \
		python3 scripts/mobile_web_ssh_flow_simulator.py --server "http://127.0.0.1:$(PORT)" --auto-approve --json; \
	fi
