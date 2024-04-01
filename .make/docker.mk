## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/docker.mk

DOCKER_BUILD_PATH ?= .
DOCKER_DEV_FEATURES ?= ""

.help-docker:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Docker targets$(SGR0)"
	@echo "  $(BOLD)docker-build$(SGR0)   -- Run docker build to create new image"
	@echo "  $(BOLD)docker-run$(SGR0)     -- Run docker container as a daemon, binding port $(HOST_PORT):$(DOCKER_PORT)"
	@echo "  $(BOLD)docker-stop$(SGR0)    -- Run 'docker kill $${DOCKER_NAME}-run' to stop our docker after running"

docker-build:
	@DOCKER_BUILDKIT=1 docker build --build-arg PACKAGE_NAME=$(PACKAGE_NAME) --tag $(PACKAGE_NAME):latest $(DOCKER_BUILD_PATH)

docker-build-dev:
	@DOCKER_BUILDKIT=1 docker build --build-arg PACKAGE_NAME=$(PACKAGE_NAME) --build-arg ENABLE_FEATURES=${DOCKER_DEV_FEATURES} --tag $(PACKAGE_NAME):dev $(DOCKER_BUILD_PATH)

docker-run: docker-stop
	# Run docker container as a daemon and map a port
	@docker compose up web-server -d

docker-stop:
	@docker compose down || true
