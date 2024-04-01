## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/python.mk

PYTHON_PATH=""

.help-python:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Python$(SGR0)"
	@echo "  $(BOLD)python-test$(SGR0) -- Run 'pycodestyle --exclude='**/.cargo' .'"
	@echo "                 to validate python files against pep8 style guide."
	@echo "  $(BOLD)python-tidy$(SGR0) -- Run 'autopep8 --in-place --recursive --exclude='**/.cargo' .' to fix python style formats if needed."

# Python / pep8 targets
python-test: docker-pull
	@echo "$(CYAN)Formatting and checking Python files with pep8 style...$(SGR0)"
	@$(call docker_run,pycodestyle --exclude="$(PYTHON_PATH).cargo" .)

python-tidy: docker-pull
	@echo "$(CYAN)Running python file formatting fixes...$(SGR0)"
	@$(call docker_run, autopep8 --in-place --recursive --exclude="$(PYTHON_PATH).cargo" --exclude="$(PYTHON_PATH)target" .)
