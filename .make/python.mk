## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/python.mk

PYTHON_PATH=""

.help-python:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Python$(SGR0)"
	@echo "  $(BOLD)python-test$(SGR0) -- Run 'yapf -r -i -vv --style google --exclude '**/.cargo/registry' .'"
	@echo "                 to validate python files against Google style guide."
	@echo "                 Run 'flake8 --exclude '**/.cargo/registry' .' to validate python files against flake8 style guide."
	@echo "  $(BOLD)python-tidy$(SGR0) -- Run 'black --extend-exclude .cargo ' to fix python style formats if needed."

# Python / yapf, flake8 targets
python-test: docker-pull
	@echo "$(CYAN)Formatting and checking Python files with Google style...$(SGR0)"
	@$(call docker_run,yapf -r -i -vv --style google --exclude "$(PYTHON_PATH).cargo/registry" .)
	@echo "$(CYAN)Formatting and checking Python files with flake8 style...$(SGR0)"
	@$(call docker_run,flake8 --exclude "$(PYTHON_PATH).cargo/registry" .)

python-tidy: docker-pull
	@echo "$(CYAN)Running python file formatting fixes...$(SGR0)"
	@$(call docker_run,black --extend-exclude $(PYTHON_PATH).cargo .)
