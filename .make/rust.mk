## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/rust.mk

RUST_IMAGE_NAME         ?= ghcr.io/arrow-air/tools/arrow-rust
RUST_IMAGE_TAG          ?= 1.1
CARGO_MANIFEST_PATH     ?= Cargo.toml
CARGO_INCREMENTAL       ?= 1
RUSTC_BOOTSTRAP         ?= 0
RELEASE_TARGET          ?= x86_64-unknown-linux-musl
PUBLISH_DRY_RUN         ?= 1
OUTPUTS_PATH            ?= $(SOURCE_PATH)/out
ADDITIONAL_OPT          ?=
PACKAGE_FEATURES        ?= ""

# function with a generic template to run docker with the required values
# Accepts $1 = command to run, $2 = additional command flags (optional)
ifeq ("$(CARGO_MANIFEST_PATH)", "")
cargo_run = echo "$(BOLD)$(YELLOW)No Cargo.toml found in any of the subdirectories, skipping cargo check...$(SGR0)"
else
cargo_run = docker run \
	--name=$(DOCKER_NAME)-$@ \
	--rm \
	--user `id -u`:`id -g` \
	--workdir=/usr/src/app \
	$(ADDITIONAL_OPT) \
	-v "$(SOURCE_PATH)/:/usr/src/app" \
	-v "$(SOURCE_PATH)/.cargo/registry:/usr/local/cargo/registry" \
	-e CARGO_INCREMENTAL=$(CARGO_INCREMENTAL) \
	-e RUSTC_BOOTSTRAP=$(RUSTC_BOOTSTRAP) \
	-t $(RUST_IMAGE_NAME):$(RUST_IMAGE_TAG) \
	cargo $(1) --manifest-path "$(CARGO_MANIFEST_PATH)" $(2)
endif

rust-docker-pull:
	@echo docker pull -q $(RUST_IMAGE_NAME):$(RUST_IMAGE_TAG)

.help-rust:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Rust$(SGR0)"
	@echo "  $(YELLOW)All cargo commands will use '--manifest-path $(CARGO_MANIFEST_PATH)'$(SGR0)"
	@echo "  $(BOLD)rust-build$(SGR0)       -- Run 'cargo build'"
	@echo "  $(BOLD)rust-release$(SGR0)     -- Run 'cargo build --release --target RELEASE_TARGET'"
	@echo "                     (RELEASE_TARGET=$(RELEASE_TARGET))"
	@echo "  $(BOLD)rust-publish$(SGR0)     -- Run 'cargo publish --package $(PUBLISH_PACKAGE_NAME)'"
	@echo "                     uses '--dry-run' by default, automation uses PUBLISH_DRY_RUN=0 to upload crate"
	@echo "  $(BOLD)rust-clean$(SGR0)       -- Run 'cargo clean'"
	@echo "  $(BOLD)rust-check$(SGR0)       -- Run 'cargo check'"
	@echo "  $(BOLD)rust-test$(SGR0)        -- Run 'cargo test --all'"
	@echo "  $(BOLD)rust-example-ARG$(SGR0) -- Run 'cargo run --example ARG' (replace ARG with example name)"
	@echo "  $(BOLD)rust-clippy$(SGR0)      -- Run 'cargo clippy --all -- -D warnings'"
	@echo "  $(BOLD)rust-fmt$(SGR0)         -- Run 'cargo fmt --all -- --check' to check rust file formats."
	@echo "  $(BOLD)rust-tidy$(SGR0)        -- Run 'cargo fmt --all' to fix rust file formats if needed."
	@echo "  $(BOLD)rust-doc$(SGR0)         -- Run 'cargo doc --all' to produce rust documentation."
	@echo "  $(BOLD)rust-openapi$(SGR0)     -- Run 'cargo run -- --api ./out/$(PACKAGE_NAME)-openapi.json'."
	@echo "  $(BOLD)rust-validate-openapi$(SGR0) -- Run validation on the ./out/$(PACKAGE_NAME)-openapi.json."
	@echo "  $(BOLD)rust-grpc-api$(SGR0)    -- Generate a $(PACKAGE_NAME)-grpc-api.json from proto/*.proto files."
	@echo "  $(BOLD)rust-coverage$(SGR0)    -- Run tarpaulin unit test coverage report"
	@echo "  $(CYAN)Combined targets$(SGR0)"
	@echo "  $(BOLD)rust-test-all$(SGR0)    -- Run targets: rust-build rust-check rust-test rust-clippy rust-fmt"
	@echo "  $(BOLD)rust-all$(SGR0)         -- Run targets; rust-clean rust-test-all rust-release"

# Rust / cargo targets
check-cargo-registry:
	if [ ! -d "$(SOURCE_PATH)/.cargo/registry" ]; then mkdir -p "$(SOURCE_PATH)/.cargo/registry" ; fi
check-logs-dir:
	if [ ! -d "$(SOURCE_PATH)/logs" ]; then mkdir -p "$(SOURCE_PATH)/logs" ; fi

.SILENT: check-cargo-registry check-logs-dir rust-docker-pull

rust-build: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo build...$(SGR0)"
	@$(call cargo_run,build, --features $(PACKAGE_FEATURES) )

rust-release: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo build --release...$(SGR0)"
	@$(call cargo_run,build, --features $(PACKAGE_FEATURES) --release --target $(RELEASE_TARGET))

rust-publish: rust-build
	@echo "$(CYAN)Running cargo publish --package $(PUBLISH_PACKAGE_NAME)...$(SGR0)"
ifeq ("$(PUBLISH_DRY_RUN)", "0")
	@echo $(call cargo_run,publish,--package $(PUBLISH_PACKAGE_NAME) --target $(RELEASE_TARGET))
else
	@$(call cargo_run,publish,--dry-run --package $(PUBLISH_PACKAGE_NAME) --target $(RELEASE_TARGET))
endif

rust-clean: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo clean...$(SGR0)"
	@$(call cargo_run,clean)

rust-check: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo check...$(SGR0)"
	@$(call cargo_run,check)

rust-test-features: $(EXCLUSIVE_FEATURES_TEST)
$(EXCLUSIVE_FEATURES_TEST):
	@echo "$(CYAN)Running cargo test for feature $@...$(SGR0)"
	@$(call cargo_run,test,--features $@ --all)
rust-test: check-cargo-registry rust-docker-pull rust-test-features
	@echo "$(CYAN)Running cargo test...$(SGR0)"
	@$(call cargo_run,test,--features $(PACKAGE_FEATURES) --all)

rust-example-%: EXAMPLE_TARGET=$*
rust-example-%: check-cargo-registry check-logs-dir rust-docker-pull
	@docker compose run \
		--user `id -u`:`id -g` \
		--rm \
		-e CARGO_INCREMENTAL=1 \
		-e RUSTC_BOOTSTRAP=0 \
		-e EXAMPLE_TARGET=$(EXAMPLE_TARGET) \
		-e SERVER_PORT_GRPC=$(DOCKER_PORT_GRPC) \
		-e SERVER_PORT_REST=$(DOCKER_PORT_REST) \
		-e SERVER_HOSTNAME=$(DOCKER_NAME)-example-server \
		example && docker compose down

rust-clippy: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running clippy...$(SGR0)"
	@$(call cargo_run,clippy,--all -- -D warnings)

rust-fmt: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running and checking Rust codes formats...$(SGR0)"
	@$(call cargo_run,fmt,--all -- --check)

rust-tidy: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running rust file formatting fixes...$(SGR0)"
	@$(call cargo_run,fmt,--all)

rust-doc: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo doc...$(SGR0)"
	@$(call cargo_run,doc,--no-deps)

rust-openapi: check-cargo-registry rust-docker-pull rust-build
	@echo "$(CYAN)Generating openapi documentation...$(SGR0)"
	mkdir -p $(OUTPUTS_PATH)
	@$(call cargo_run,run,-- --openapi ./out/$(PACKAGE_NAME)-openapi.json)

rust-validate-openapi: rust-openapi
	@docker run \
		--rm \
		-v $(OUTPUTS_PATH):/out \
		jeanberu/swagger-cli \
		swagger-cli validate /out/$(PACKAGE_NAME)-openapi.json

rust-grpc-api:
	@echo "$(CYAN)Generating GRPC documentation...$(SGR0)"
	mkdir -p $(OUTPUTS_PATH)
	@docker run \
		--rm \
		--user `id -u`:`id -g` \
		-v $(SOURCE_PATH)/proto:/protos \
		-v $(OUTPUTS_PATH):/out \
		pseudomuto/protoc-gen-doc \
		--doc_opt=json,$(PACKAGE_NAME)-grpc-api.json

rust-coverage: ADDITIONAL_OPT = --security-opt seccomp='unconfined'
rust-coverage: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Rebuilding and testing with profiling enabled...$(SGR0)"
	@mkdir -p coverage/
	@$(call cargo_run,tarpaulin,\
		--workspace -l --include-tests --tests --no-fail-fast \
		--features $(PACKAGE_FEATURES) --skip-clean -t 600 --out Lcov \
		--output-dir coverage/)
	@sed -e "s/\/usr\/src\/app\///g" -i coverage/lcov.info

rust-test-all: rust-build rust-check rust-test rust-clippy rust-fmt rust-doc
rust-all: rust-clean rust-test-all rust-release
