.PHONY: build run test tidy clean

BINARY := ob

build:
	go build -o bin/$(BINARY) ./cmd/$(BINARY)

run:
	go run ./cmd/$(BINARY)

run-local:
	ENV=local go run ./cmd/$(BINARY)

test:
	go test ./...

tidy:
	go mod tidy

clean:
	rm -rf bin/
