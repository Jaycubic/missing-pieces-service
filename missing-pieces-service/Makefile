# missing-pieces-service/Makefile
# Usage:  make assets | make build | make start | make stop | make restart | make logs | make health

SERVICE = missing-pieces-service
BINARY  = target/release/$(SERVICE)

.PHONY: assets build start stop restart status logs clean health install migrate

## Compile Tailwind into public/styles.css (Node only runs here, at build time)
assets:
	cd web && npm install --no-audit --no-fund && npm run build
	@echo "✓ public/styles.css rebuilt"

## Build release binary (also rebuilds CSS first)
build: assets
	cargo build --release
	@echo "✓ Binary ready at $(BINARY)"

## Start systemd service
start:
	sudo systemctl start $(SERVICE)
	@echo "✓ $(SERVICE) started"

## Stop systemd service
stop:
	sudo systemctl stop $(SERVICE)
	@echo "✓ $(SERVICE) stopped"

## Rebuild + restart (typical deploy)
restart: build
	sudo systemctl restart $(SERVICE)
	sudo systemctl status $(SERVICE) --no-pager
	@echo "✓ $(SERVICE) restarted with new binary and assets"

## Show service status
status:
	sudo systemctl status $(SERVICE) --no-pager

## Stream live logs
logs:
	sudo journalctl -u $(SERVICE) -f

## Remove compiled output
clean:
	cargo clean
	rm -f public/styles.css
	@echo "✓ target/ and compiled CSS removed"

## Check health endpoint (migrations run automatically on startup too)
health:
	curl -s http://127.0.0.1:8091/health | python3 -m json.tool

## Install systemd unit (first-time setup only)
install:
	sudo cp deploy/missing-pieces-service.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable $(SERVICE)
	@echo "✓ systemd unit installed and enabled"
