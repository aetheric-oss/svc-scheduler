## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/commitlint.mk

COMMITLINT_CHECK :=

.help-commitlint:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Commitlint$(SGR0)"
	@echo "  $(BOLD)commitlint-test$(SGR0)  -- Run 'commitlint --edit $(COMMITLINT_CHECK)'"
	@echo "                      to validate commit messages are following the conventional commit standards."

commitlint-test: docker-pull
	@echo "$(CYAN)Checking for commit message errors...$(SGR0)"
	@$(call docker_run,commitlint --edit $(COMMITLINT_CHECK))
