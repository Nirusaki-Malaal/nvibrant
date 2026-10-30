.DEFAULT_GOAL := build
SHELL := /bin/bash
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
PREFIX ?= $(HOME)/.local
BUILD_DIR ?= $(ROOT)/build
PYTHON ?= /usr/bin/python3
DRIVER_VERSION ?=
JOBS ?=
export NVIBRANT_PYTHON := $(PYTHON)

# Keep one build/install implementation for Make, local use and curl bootstrap.
BUILD_FLAGS = --prefix "$(PREFIX)" --build-dir "$(BUILD_DIR)" $(if $(DRIVER_VERSION),--driver-version "$(DRIVER_VERSION)") $(if $(JOBS),--jobs "$(JOBS)")

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
# GUI smoke tests require Xvfb; unit tests run without a display or NVIDIA GPU.
test:
	cd "$(ROOT)" && "$(PYTHON)" -m unittest discover -s tests -v
test-gui:
	cd "$(ROOT)" && xvfb-run -a env GDK_BACKEND=x11 GSK_RENDERER=cairo "$(PYTHON)" tests/smoke_gui.py
	cd "$(ROOT)" && xvfb-run -a env GDK_BACKEND=x11 GSK_RENDERER=cairo "$(PYTHON)" tests/smoke_settings.py
clean:
	meson compile -C "$(BUILD_DIR)" --clean
help:
	@printf '%s\n' 'make [build]   Build a native x86-64 or ARM64 backend' 'make install   Install GUI, CLI and desktop launcher (PREFIX=~/.local)' 'make uninstall Remove only this installation' 'make deps      Install distro dependencies (may use sudo)' 'make check     Check build and GTK runtime dependencies' 'make test      Run unit and installer regression tests' 'make test-gui  Run real GTK smoke checks under Xvfb' 'make clean     Clean Meson build output' 'Options: PREFIX=/path BUILD_DIR=/path DRIVER_VERSION=615.78.08 JOBS=4'
