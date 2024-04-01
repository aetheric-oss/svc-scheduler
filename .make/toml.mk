## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/toml.mk

TOML_FILES ?= $(shell find . -type f -iname '*.toml' ! -path "./target")

.help-toml:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)TOML$(SGR0)"
	@echo "  $(BOLD)toml-test$(SGR0)   -- Run 'taplo format --check' to validate TOML file formats."
	@echo "  $(BOLD)toml-tidy$(SGR0)   -- Run 'taplo format' to fix TOML file formats if needed."

toml-test: docker-pull
ifeq ("$(TOML_FILES)", "")
	@echo "$(YELLOW)No toml files found, skipping formatting tests...$(SGR0)"
else
	@echo "$(CYAN)Running toml file formatting tests...$(SGR0)"
	@$(call docker_run,taplo format --check)
endif

toml-tidy: docker-pull
ifeq ("$(TOML_FILES)", "")
	@echo "$(YELLOW)No toml files found, skipping formatting fixes...$(SGR0)"
else
	@echo "$(CYAN)Running toml file formatting fixes...$(SGR0)"
	@$(call docker_run,taplo format)
endif
