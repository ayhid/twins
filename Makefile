BINARY   := twins
MODULE   := github.com/ayhid/twins
VERSION  ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
LDFLAGS  := -s -w -X $(MODULE)/internal/cli.version=$(VERSION)

.PHONY: build test cover lint bench run clean

build:
	go build -ldflags '$(LDFLAGS)' -o bin/$(BINARY) ./cmd/twins

test:
	go test -race -count=1 ./...

cover:
	go test -count=1 -coverprofile=coverage.out ./...
	go tool cover -func=coverage.out | tail -1

lint:
	go vet ./...

bench:
	go test -run '^$$' -bench . -benchmem ./internal/...

run: build
	./bin/$(BINARY)

clean:
	rm -rf bin dist coverage.out
