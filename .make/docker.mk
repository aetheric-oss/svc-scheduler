## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/docker.mk

.help-docker:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Docker targets$(SGR0)"
	@echo "  $(BOLD)docker-compose-run-<target>$(SGR0) -- Run 'docker compose run --rm <target>' to run the provided target."
	@echo "  $(BOLD)docker-build-<target>$(SGR0)       -- Run 'docker compose build <target>' to build the docker image for the provided target."

# Runs a `docker compose run ..` command on provided target.
# In order to shut down dependencies, this command assumes the dependencies have a profile set
# in the compose file which is corresponding to the target name provided for this command.
docker-compose-run-%: DOCKER_COMPOSE_TARGET=$*
docker-compose-run-%:
	@echo "$(BOLD)$(CYAN) Running docker compose for target [$(DOCKER_COMPOSE_TARGET)].$(SGR0)"
	@docker compose run --rm $(DOCKER_COMPOSE_TARGET) \
		; docker compose --profile $(DOCKER_COMPOSE_TARGET) down --remove-orphans

docker-build-%:
	@echo "$(BOLD)$(CYAN) Running docker build for target [$*].$(SGR0)"
	@docker compose build $*
