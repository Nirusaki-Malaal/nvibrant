.DEFAULT_GOAL := build
SHELL := /bin/bash
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
PREFIX ?= $(HOME)/.local
BUILD_DIR ?= $(ROOT)/build
DRIVER_VERSION ?=
JOBS ?= 2
BUILD_FLAGS = --prefix "$(PREFIX)" --build-dir "$(BUILD_DIR)" $(if $(DRIVER_VERSION),--driver-version "$(DRIVER_VERSION)") --jobs "$(JOBS)"
.PHONY: all build install uninstall deps check test test-gui clean help
all: build
build:
	bash "$(ROOT)/build.sh" build $(BUILD_FLAGS)
install:
	bash "$(ROOT)/build.sh" install $(BUILD_FLAGS)
uninstall:
	bash "$(ROOT)/build.sh" uninstall --prefix "$(PREFIX)"
deps:
	bash "$(ROOT)/build.sh" deps
check:
	bash "$(ROOT)/build.sh" check
test:
	cd "$(ROOT)" && cargo test --locked -j "$(JOBS)"
test-gui: build
	xvfb-run -a env GDK_BACKEND=x11 GSK_RENDERER=cairo "$(BUILD_DIR)/nvibrant" --smoke-test
clean:
	cd "$(ROOT)" && cargo clean
help:
	@printf '%s\n' 'make build     Compile the Rust GUI and NVIDIA backend' 'make install   Install the binary, desktop icon and login restoration' 'make uninstall Remove the application and managed login entry' 'make deps      Install distro build dependencies (may use sudo)' 'make check     Check Rust, GTK and libclang dependencies' 'make test      Run Rust settings, restore, CLI and installer tests' 'make test-gui  Exercise the real Rust GTK interface under Xvfb' 'Options: PREFIX=/path BUILD_DIR=/path DRIVER_VERSION=615.78.08 JOBS=2'
