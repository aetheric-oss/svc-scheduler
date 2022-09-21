SHELL := /bin/bash

DOCKER_NAME      := arrow-lib-scheduler
IMAGE_NAME       := lib-scheduler
BUILD_IMAGE_NAME := ghcr.io/arrow-air/tools/arrow-rust
BUILD_IMAGE_TAG  := latest

DOCKER_PORT      := 8080
HOST_PORT        := 8002


# We might not have a Cargo.toml file in the root dir
CARGO_MANIFEST_PATH ?= $(shell find -maxdepth 2 -name Cargo.toml)
CARGO_INCREMENTAL   ?= 1
RUSTC_BOOTSTRAP     ?= 0
RELEASE_TARGET      ?= x86_64-unknown-linux-musl

# Style templates for console output.
GREEN  := $(shell echo -e `tput setaf 2`)
YELLOW := $(shell echo -e `tput setaf 3`)
CYAN   := $(shell echo -e `tput setaf 6`)
NC     := $(shell echo -e `tput setaf 9`)
BOLD   := $(shell echo -e `tput bold`)
SMUL   := $(shell echo -e `tput smul`)
SGR0   := $(shell echo -e `tput sgr0`)

# function with a generic template to run docker with the required values
# Accepts $1 = command to run, $2 = additional flags (optional)
docker_run = docker run \
	--name=$(DOCKER_NAME)-$@ \
	--rm \
	--user `id -u`:`id -g` \
	-e CARGO_INCREMENTAL=$(CARGO_INCREMENTAL) \
	-e RUSTC_BOOTSTRAP=$(RUSTC_BOOTSTRAP) \
	-v "$(PWD):/usr/src/app" \
	-v "$(PWD)/.cargo/registry:/usr/local/cargo/registry" \
	$(2) \
	-t $(BUILD_IMAGE_NAME):$(BUILD_IMAGE_TAG) \
	$(1)

ifeq ("$(CARGO_MANIFEST_PATH)", "")
cargo_run = echo "$(BOLD)$(YELLOW)No Cargo.toml found in any of the subdirectories, skipping cargo check...$(NC)$(SGR0)"
else
cargo_run = $(call docker_run,cargo $(1) --manifest-path "$(CARGO_MANIFEST_PATH)" $(2))
endif

help:
	@echo ""
	@echo "$(BOLD)$(CYAN)Available targets$(NC)$(SGR0)"
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Rust$(NC)$(SGR0)"
	@echo "  $(YELLOW)All cargo commands will use '--manifest-path $(CARGO_MANIFEST_PATH)'$(NC)"
	@echo "  $(BOLD)build$(SGR0)       -- Run 'cargo build'"
	@echo "  $(BOLD)release$(SGR0)     -- Run 'cargo build --release --target RELEASE_TARGET'"
	@echo "                 (RELEASE_TARGET=$(RELEASE_TARGET))"

	@echo "  $(BOLD)clean$(SGR0)       -- Run 'cargo clean'"
	@echo "  $(BOLD)rust-check$(SGR0)  -- Run 'cargo check'"
	@echo "  $(BOLD)rust-test$(SGR0)   -- Run 'cargo test --all'"
	@echo "  $(BOLD)rust-clippy$(SGR0) -- Run 'cargo clippy --all -- -D warnings'"
	@echo "  $(BOLD)rust-fmt$(SGR0)    -- Run 'cargo fmt --all -- --check' to check rust file formats."
	@echo "  $(BOLD)rust-tidy$(SGR0)   -- Run 'cargo fmt --all' to fix rust file formats if needed."
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)TOML$(NC)$(SGR0)"
	@echo "  $(BOLD)toml-test$(SGR0)   -- Run 'taplo format --check' to validate TOML file formats."
	@echo "  $(BOLD)toml-tidy$(SGR0)   -- Run 'taplo format' to fix TOML file formats if needed."
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Python$(NC)$(SGR0)"
	@echo "  $(BOLD)python-test$(SGR0) -- Run 'yapf -r -i -vv --style google --exclude '.cargo/registry' .'"
	@echo "                 to validate python files against Google style guide."
	@echo "                 Run 'flake8 --exclude '.cargo/registry' .' to validate python files against flake8 style guide."
	@echo "  $(BOLD)python-tidy$(SGR0) -- Run 'black --extend-exclude .cargo ' to fix python style formats if needed."
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)CSpell$(NC)$(SGR0)"
	@echo "  $(BOLD)cspell-test$(SGR0) -- Run 'cspell --words-only --unique "**/**" -c .cspell.config.yaml'"
	@echo "                 to validate files are not containing any spelling errors."
	@echo "  $(BOLD)cspell-add-words$(SGR0) -- Run 'cspell --words-only --unique "**/**" -c .cspell.config.yaml 2> /dev/null |"
	@echo "                      sort -f >> .cspell.project-words.txt'"
	@echo "                      to add remaining words to the project's cspell ignore list"
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Combined targets$(NC)$(SGR0)"
	@echo "  $(BOLD)test$(SGR0) -- Run targets; rust-check rust-test rust-clippy rust-fmt toml-test python-test"
	@echo "  $(BOLD)tidy$(SGR0) -- Run targets; rust-tidy toml-tidy python-tidy"

	@echo "  $(BOLD)all$(SGR0)  -- Run targets; test build release docker-build"
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Docker targets$(NC)$(SGR0)"
	@echo "  $(BOLD)docker-build$(SGR0)-- Run docker build to create new image"
	@echo "  $(BOLD)docker-run$(SGR0)  -- Run docker container as a daemon, binding port $(HOST_PORT):$(DOCKER_PORT)"
	@echo "  $(BOLD)docker-stop$(SGR0) -- Run 'docker kill ${DOCKER_NAME}-run' to stop our docker after running"
	@echo "  $(BOLD)all$(SGR0)  -- Run targets; test build release run"


.SILENT: check-cargo-registry docker-pull

docker-pull:
	@docker pull -q $(BUILD_IMAGE_NAME):$(BUILD_IMAGE_TAG)

# Rust / cargo targets
check-cargo-registry:
	if [ ! -d "$(PWD)/.cargo/registry" ]; then mkdir -p "$(PWD)/.cargo/registry" ; fi

build: check-cargo-registry docker-pull
	@$(call cargo_run,build)

release: docker-pull
	@$(call cargo_run,build,--release --target $(RELEASE_TARGET))

publish: docker-pull
	@$(call cargo_run,publish,--dry-run --target $(RELEASE_TARGET))

clean: check-cargo-registry docker-pull
	@$(call cargo_run,clean)

rust-check: check-cargo-registry docker-pull
	@$(call cargo_run,check)

rust-test: check-cargo-registry docker-pull
	@$(call cargo_run,test,--all)

rust-clippy: check-cargo-registry docker-pull
	@$(call cargo_run,clippy,--all -- -D warnings)

rust-fmt: check-cargo-registry docker-pull
	@echo "$(YELLOW)Running and checking Rust codes formats...$(NC)"
	@$(call cargo_run,fmt,--all -- --check)

rust-tidy: check-cargo-registry docker-pull
	@echo "$(YELLOW)Running rust file formatting fixes...$(NC)"
	@$(call cargo_run,fmt,--all)

# TOML / taplo targets
toml-test: docker-pull
	@echo "$(YELLOW)Running toml file formatting tests...$(NC)"
	@$(call docker_run,taplo format --check)

toml-tidy: docker-pull
	@echo "$(YELLOW)Running toml file formatting fixes...$(NC)"
	@$(call docker_run,taplo format)

# Python / yapf, flake8 targets
python-test: docker-pull
	@echo "$(YELLOW)Formatting and checking Python files with Google style...$(NC)"
	@$(call docker_run,yapf -r -i -vv --style google --exclude '.cargo/registry' .)
	@echo "$(YELLOW)Formatting and checking Python files with flake8 style...$(NC)"
	@$(call docker_run,flake8 --exclude '.cargo/registry' .)

python-tidy: docker-pull
	@echo "$(YELLOW)Running python file formatting fixes...$(NC)"
	@$(call docker_run,black --extend-exclude .cargo .)

# editorconfig targets
editorconfig-test:
	@echo "$(YELLOW)Checking if the codebase is compliant with the .editorconfig file...$(NC)"
	@docker run \
		--name=$(DOCKER_NAME)-$@ \
		--rm \
		--user `id -u`:`id -g` \
		-w "/usr/src/app" \
		-v "$(PWD):/usr/src/app" \
		-t mstruebing/editorconfig-checker

# cspell targets
cspell-test: docker-pull
	@echo "$(YELLOW)Checking for spelling errors...$(NC)"
	@$(call docker_run,cspell --words-only --unique "**/**" -c .cspell.config.yaml)

# cspell add words
cspell-add-words: docker-pull
	@echo "$(YELLOW)Adding words to the project's cspell word list...$(NC)"
	@$(call docker_run,sh -c 'cspell --words-only --unique "**/**" -c .cspell.config.yaml 2> /dev/null | sort -f >> .cspell.project-words.txt')

# Combined targets
test: rust-check rust-test rust-clippy rust-fmt toml-test python-test
tidy: rust-tidy toml-tidy python-tidy

# Docker targets
docker-build: docker-pull
	@docker build --build-arg PACKAGE_NAME --tag $(IMAGE_NAME):latest ../

docker-run: docker-stop
	# Run docker container as a daemon and map a port
	@docker run --rm -d -p $(HOST_PORT):$(DOCKER_PORT) --name=$(DOCKER_NAME)-run $(IMAGE_NAME):latest
	@echo "$(YELLOW)Docker running and listening to http://localhost:$(HOST_PORT)$(NC)"

docker-stop:
	@docker kill $(DOCKER_NAME)-run || true

all: test build release docker-build

