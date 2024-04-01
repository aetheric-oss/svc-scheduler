## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/editorconfig.mk

.help-editorconfig:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Editorconfig$(SGR0)"
	@echo "  $(BOLD)editorconfig-test$(SGR0) -- Run editorconfig test to validate all file formats are valid"
	@echo "  $(BOLD)editorconfig-tidy$(SGR0) -- Run editorconfig tidy to fix all file formats if needed"

editorconfig-test:
	@echo "$(CYAN)Checking if the codebase is compliant with the .editorconfig file...$(SGR0)"
	@docker run \
		--name=$(DOCKER_NAME)-$@ \
		--rm \
		--user `id -u`:`id -g` \
		-w "/usr/src/app" \
		-v "$(PWD):/usr/src/app" \
		-t mstruebing/editorconfig-checker

editorconfig-tidy: docker-pull
	@echo "$(CYAN)Running editorconfig formatting fixes...$(SGR0)"
	@$(call docker_run,sh -c 'eclint fix .')
