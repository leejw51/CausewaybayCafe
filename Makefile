.DEFAULT_GOAL := help
export NO_COLOR := true
CAFE_BIND ?= 0.0.0.0
CAFE_FRONTEND_BIND ?= 0.0.0.0
CAFE_FRONTEND_PORT ?= 8791
export CAFE_BIND CAFE_FRONTEND_BIND CAFE_FRONTEND_PORT
.PHONY: help build web start normaltest stop status stressdat simulationtest stresstest test test-e2e test-setup check fmt test-http3 test-meta test-meta-photo
help:
	@echo 'make start       Start in Simulation mode for easy testing'
	@echo 'make normaltest  Start in Live mode without simulation'
	@echo 'make stop        Stop this checkout’s frontend and backend'
	@echo 'make simulationtest  Automatically order, prepare, serve and pay for 12 test orders'
	@echo 'make stresstest  Rust + Playwright + 64 complete orders with 8 concurrent clients'
	@echo 'make stressdat   Start a local-only demo with simulated orders'
	@echo 'make status      Show supervisor status'
build:
	cargo build -p cafe-server --bins
web:
	trunk build
start: build web
	python3 scripts/supervisor.py stop
	python3 scripts/supervisor.py start --mode simulation
normaltest: build web
	python3 scripts/supervisor.py stop
	python3 scripts/supervisor.py start --mode live
stop:
	python3 scripts/supervisor.py stop
status:
	python3 scripts/supervisor.py status
stressdat: build web
	python3 scripts/supervisor.py stop
	python3 scripts/supervisor.py start --simulation
test:
	cargo test -p cafe-shared -p cafe-server
test-setup:
	@test -d node_modules/@playwright/test || npm ci
	npx playwright install chromium webkit
test-e2e: build web test-setup
	npm run test:e2e
simulationtest: build web
	python3 scripts/e2e_server.py --scenario simulation
stresstest:
	$(MAKE) test
	$(MAKE) test-e2e
	python3 scripts/e2e_server.py --scenario stress
check:
	cargo check -p cafe-server
	trunk build
fmt:
	cargo fmt --all
test-http3:
	cargo test -p cafe-server --test http3 -- --ignored
test-meta:
	cargo test -p cafe-server --test workflows live_meta_chat -- --ignored
test-meta-photo:
	cargo test -p cafe-server --test workflows live_photo_menu_and_image -- --ignored --nocapture

# Local HTTPS TCP + HTTP/3 UDP on port 8792. Include each address used by devices.
.PHONY: tls
TLS_HOSTS ?= localhost
tls:
	python3 scripts/local_tls.py $(foreach host,$(TLS_HOSTS),--host $(host))
