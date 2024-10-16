## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/rust.mk

DOCKER_NAME         ?= ${PACKAGE_NAME}
DOCKER_IMAGE_NAME   ?= $(PACKAGE_NAME)
DOCKER_USER_ID      ?= $(shell echo `id -u`)
DOCKER_GROUP_ID     ?= $(shell echo `id -g`)
CARGO_MANIFEST_PATH ?= Cargo.toml
CARGO_INCREMENTAL   ?= 1
RUSTC_BOOTSTRAP     ?= 0
RELEASE_TARGET      ?= x86_64-unknown-linux-musl
PUBLISH_DRY_RUN     ?= 1
OUTPUTS_PATH        ?= $(SOURCE_PATH)/out
ADDITIONAL_OPT      ?=

PACKAGE_BUILD_FEATURES   ?= ""
PACKAGE_RELEASE_FEATURES ?= ""

# Can contain quotes, but we don't want quotes
EXCLUSIVE_FEATURES_TEST  := $(shell echo ${EXCLUSIVE_FEATURES_TEST})
COMMA := ,

# function with a generic template to run docker with the required values
# Accepts $1 = command to run, $2 = additional command flags (optional)
ifeq ("$(CARGO_MANIFEST_PATH)", "")
	cargo_run = echo "$(BOLD)$(YELLOW)No Cargo.toml found in any of the subdirectories, skipping cargo check...$(SGR0)"
else
	cargo_run = docker run \
		--name=$(DOCKER_NAME)-$(subst $(COMMA),_,$@) \
		--rm \
		--user $(DOCKER_USER_ID):$(DOCKER_GROUP_ID) \
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
	@docker pull -q $(RUST_IMAGE_NAME):$(RUST_IMAGE_TAG)
latest-docker-pull:
	@docker pull -q $(DOCKER_IMAGE_NAME):latest 2>/dev/null || true
dev-docker-pull:
	@docker pull -q $(DOCKER_IMAGE_NAME):dev 2>/dev/null || true

.help-rust:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Rust$(SGR0)"
	@echo "  $(YELLOW)All cargo commands will use '--manifest-path $(CARGO_MANIFEST_PATH)'$(SGR0)"
	@echo "  $(BOLD)rust-build$(SGR0)       -- Run 'cargo build'"
	@echo "  $(BOLD)rust-test$(SGR0)        -- Run 'cargo test --workspace'"
	@echo "  $(BOLD)rust-release$(SGR0)     -- Run 'cargo build --release --target RELEASE_TARGET'"
	@echo "                     (RELEASE_TARGET=$(RELEASE_TARGET))"
	@echo "  $(BOLD)rust-publish$(SGR0)     -- Run 'cargo publish --package $(PUBLISH_PACKAGE_NAME)'"
	@echo "                     uses '--dry-run' by default, automation uses PUBLISH_DRY_RUN=0 to upload crate"
	@echo "  $(BOLD)rust-clean$(SGR0)       -- Run 'cargo clean'"
	@echo "  $(BOLD)rust-check$(SGR0)       -- Run 'cargo check'"
	@echo "  $(BOLD)rust-clippy$(SGR0)      -- Run 'cargo clippy --workspace -- -D warnings'"
	@echo "  $(BOLD)rust-fmt$(SGR0)         -- Run 'cargo fmt --workspace -- --check' to check rust file formats."
	@echo "  $(BOLD)rust-tidy$(SGR0)        -- Run 'cargo fmt --workspace' to fix rust file formats if needed."
	@echo "  $(BOLD)rust-doc$(SGR0)         -- Run 'cargo doc --workspace' to produce rust documentation."
	@echo "  $(BOLD)rust-openapi$(SGR0)     -- Run 'cargo run -- --api ./out/$(PACKAGE_NAME)-openapi.json'."
	@echo "  $(BOLD)rust-validate-openapi$(SGR0) -- Run validation on the ./out/$(PACKAGE_NAME)-openapi.json."
	@echo "  $(BOLD)rust-grpc-api$(SGR0)    -- Generate a $(PACKAGE_NAME)-grpc-api.json from proto/*.proto files."
	@echo "  $(BOLD)rust-example-grpc$(SGR0)-- Run 'docker compose run --rm example' EXAMPLE_TARGET=grpc with stubbed backend server"
	@echo "  $(BOLD)rust-example-rest$(SGR0)-- Run 'docker compose run --rm example' EXAMPLE_TARGET=rest with stubbed backend server"
	@echo "  $(BOLD)rust-ut-coverage$(SGR0) -- Run 'docker compose run --rm ut-coverage' with 'test_util' feature enabled to generate tarpaulin unit test coverage report"
	@echo "  $(BOLD)rust-it-coverage$(SGR0) -- Run 'docker compose run --rm it-coverage' with 'test_util' feature enabled to generate tarpaulin integration test coverage report"
	@echo "  $(BOLD)rust-it-full$(SGR0)     -- Run 'docker compose run --rm it-full' to run full integration test with running backends"
	@echo "  $(CYAN)Combined targets$(SGR0)"
	@echo "  $(BOLD)rust-test-all$(SGR0)    -- Run targets: rust-build rust-check rust-test rust-clippy rust-fmt"
	@echo "  $(BOLD)rust-all$(SGR0)         -- Run targets; rust-clean rust-test-all rust-release"

# Rust / cargo targets
check-cargo-registry:
	if [ ! -d "$(SOURCE_PATH)/.cargo/registry" ]; then mkdir -p "$(SOURCE_PATH)/.cargo/registry" ; fi
check-logs-dir:
	if [ ! -d "$(SOURCE_PATH)/logs" ]; then mkdir -p "$(SOURCE_PATH)/logs" ; fi

.SILENT: check-cargo-registry check-logs-dir rust-docker-pull latest-docker-pull

rust-build: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo build with features [$(PACKAGE_BUILD_FEATURES)]...$(SGR0)"
	@$(call cargo_run,build, --features $(PACKAGE_BUILD_FEATURES) )

rust-release: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo build --release with features [$(PACKAGE_RELEASE_FEATURES)]...$(SGR0)"
	@$(call cargo_run,build, --features $(PACKAGE_RELEASE_FEATURES) --release --target $(RELEASE_TARGET))

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
	@$(call cargo_run,test,--features $@ --target-dir target/test/$@ --workspace)
rust-test: check-cargo-registry rust-docker-pull rust-test-features
	@echo "$(CYAN)Running cargo test with features [$(PACKAGE_UT_FEATURES)]...$(SGR0)"
	@$(call cargo_run,test,--features $(PACKAGE_UT_FEATURES) --workspace)

rust-clippy: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running clippy...$(SGR0)"
	@$(call cargo_run,clippy,--workspace -- -D warnings)

rust-fmt: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running and checking Rust codes formats...$(SGR0)"
	@$(call cargo_run,fmt,--all -- --check)

rust-tidy: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running rust file formatting fixes...$(SGR0)"
	@$(call cargo_run,fmt,--all)

rust-doc: check-cargo-registry rust-docker-pull
	@echo "$(CYAN)Running cargo doc...$(SGR0)"
	@$(call cargo_run,doc,--all-features --no-deps)

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
		--user $(DOCKER_USER_ID):$(DOCKER_GROUP_ID) \
		-v $(SOURCE_PATH)/proto:/protos \
		-v $(OUTPUTS_PATH):/out \
		pseudomuto/protoc-gen-doc \
		--doc_opt=json,$(PACKAGE_NAME)-grpc-api.json

rust-example-grpc: EXAMPLE_TARGET=grpc
rust-example-grpc: check-cargo-registry check-logs-dir rust-docker-pull docker-compose-run-example

rust-example-rest: EXAMPLE_TARGET=rest
rust-example-rest: check-cargo-registry check-logs-dir rust-docker-pull docker-compose-run-example

# Run unit test for this service.
# Client stubs can be enabled since we don't need to test any integrations.
rust-ut-coverage: check-cargo-registry check-logs-dir rust-docker-pull docker-compose-run-ut-coverage

# Run integration test for this service.
# Client stubs should not be enabled since we want to make actual calls to the server.
# We will run our tests against a web-server which runs with stubbed backends so we don't need to run
# any other services it depends on but are still able to validate the integration with these services.
rust-it-coverage: check-cargo-registry check-logs-dir rust-docker-pull docker-compose-run-it-coverage

# Run full integration test for this service.
# Should not use any stubs for backends so all dependencies should be started by docker compose
rust-it-full: check-cargo-registry check-logs-dir rust-docker-pull docker-compose-run-integration-test

release-checklist: docker-pull
	@echo "$(CYAN)Running release checklist...$(SGR0)"
	@$(call docker_run,python3 /usr/bin/release_checklist.py $(CHECKLIST_OPTS))

release-checklist-full: docker-pull
	$(MAKE) CHECKLIST_OPTS="-t -c" release-checklist

rust-test-all: rust-build rust-check rust-test rust-clippy rust-fmt rust-doc
rust-all: rust-clean rust-test-all rust-release
