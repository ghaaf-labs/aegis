.PHONY: help setup dev dev-status dev-down db-up db-down db-reset migrate api-check web-check quality dup clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

setup: ## First-time setup: install deps + start DB + migrate
	cp -n .env.example .env || true
	pnpm install
	$(MAKE) db-up
	sleep 3
	$(MAKE) migrate
	@echo "\n✅ Aegis ready. Run 'make dev' to start."

dev: ## Start dev servers (api + web) via the multi-agent supervisor
	scripts/dev.sh up

dev-status: ## Show dev-server status (ports, health, owner)
	scripts/dev.sh status

dev-down: ## Stop dev servers, free the ports
	scripts/dev.sh down

db-up: ## Start Postgres + Redis via Docker
	docker compose up -d postgres redis

db-down: ## Stop all Docker services
	docker compose down

db-reset: ## Wipe DB and re-migrate (destructive!)
	docker compose down -v
	$(MAKE) db-up
	sleep 3
	$(MAKE) migrate

migrate: ## Run database migrations
	cd apps/api && cargo sqlx migrate run

api-check: ## Rust fmt + clippy
	cd apps/api && cargo fmt --check && cargo clippy -- -D warnings

web-check: ## Frontend lint + type-check
	pnpm --filter @aegis/web lint && pnpm --filter @aegis/web type-check

quality: api-check web-check ## All quality gates (fmt + clippy, lint + types)

dup: ## Report duplicate code (advisory; no install)
	pnpm dlx jscpd apps/web/src apps/api/src --min-lines 8 --min-tokens 50 --reporters console

clean: ## Clean all build artifacts
	pnpm clean
	cd apps/api && cargo clean
