## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/docker.mk

DOCKER_BUILD_PATH ?= .
IMAGE_NAME        ?= ${name}

.help-docker:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Docker targets$(NC)$(SGR0)"
	@echo "  $(BOLD)docker-build$(SGR0)-- Run docker build to create new image"
	@echo "  $(BOLD)docker-run$(SGR0)  -- Run docker container as a daemon, binding port $(HOST_PORT):$(DOCKER_PORT)"
	@echo "  $(BOLD)docker-stop$(SGR0) -- Run 'docker kill $${DOCKER_NAME}-run' to stop our docker after running"

docker-build:
	@DOCKER_BUILDKIT=1 docker build --build-arg PACKAGE_NAME --tag $(IMAGE_NAME):latest $(DOCKER_BUILD_PATH)

docker-run: docker-stop
	# Run docker container as a daemon and map a port
	@docker run --rm -d -p $(HOST_PORT):$(DOCKER_PORT) --name=$(DOCKER_NAME)-run $(IMAGE_NAME):latest
	@echo "$(YELLOW)Docker running and listening to http://localhost:$(HOST_PORT)$(NC)"

docker-stop:
	@docker kill ${DOCKER_NAME}-run || true
