.PHONY: help setup dev db-up db-down db-reset migrate api-check web-check clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

setup: ## First-time setup: install deps + start DB + migrate
	cp -n .env.example .env || true
	pnpm install
	$(MAKE) db-up
	sleep 3
	$(MAKE) migrate
	@echo "\n✅ Aegis ready. Run 'make dev' to start."

dev: ## Start all dev servers (frontend + API)
	pnpm dev

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

clean: ## Clean all build artifacts
	pnpm clean
	cd apps/api && cargo clean
