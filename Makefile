.PHONY: build run run-local setup-local test tidy clean

BINARY := ob

DB_CONTAINER := ob-postgres
DB_IMAGE := postgres:16-alpine
DB_HOST := localhost
DB_PORT := 5432
DB_USER := ob
DB_PASSWORD := ob_local
DB_NAME := ob

build:
	go build -o bin/$(BINARY) ./cmd/$(BINARY)

run:
	go run ./cmd/$(BINARY)

run-local:
	ENV=local go run ./cmd/$(BINARY)

setup-local:
	@if docker ps -a --format '{{.Names}}' | grep -qx '$(DB_CONTAINER)'; then \
		echo "starting existing container $(DB_CONTAINER)"; \
		docker start $(DB_CONTAINER); \
	else \
		echo "creating container $(DB_CONTAINER)"; \
		docker run -d \
			--name $(DB_CONTAINER) \
			-e POSTGRES_USER=$(DB_USER) \
			-e POSTGRES_PASSWORD=$(DB_PASSWORD) \
			-e POSTGRES_DB=$(DB_NAME) \
			-p $(DB_PORT):5432 \
			$(DB_IMAGE); \
	fi
	@echo "postgres ready at $(DB_HOST):$(DB_PORT) (user=$(DB_USER), db=$(DB_NAME))"

test:
	go test ./...

tidy:
	go mod tidy

clean:
	rm -rf bin/
