## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/docker.mk

DOCKER_BUILD_PATH ?= .

.help-docker:
	@echo ""
	@echo "$(SMUL)$(BOLD)$(GREEN)Docker targets$(SGR0)"
	@echo "  $(BOLD)docker-build$(SGR0)   -- Run docker build to create new image"
	@echo "  $(BOLD)docker-run$(SGR0)     -- Run docker container as a daemon, binding port $(HOST_PORT):$(DOCKER_PORT)"
	@echo "  $(BOLD)docker-stop$(SGR0)    -- Run 'docker kill $${DOCKER_NAME}-run' to stop our docker after running"

docker-build:
	@DOCKER_BUILDKIT=1 docker build --build-arg PACKAGE_NAME=$(PACKAGE_NAME) --tag $(PACKAGE_NAME):latest $(DOCKER_BUILD_PATH)

docker-run: docker-stop
	# Run docker container as a daemon and map a port
	@docker run --rm -d --env-file .env -p $(HOST_PORT_GRPC):$(DOCKER_PORT_GRPC) -p $(HOST_PORT_REST):$(DOCKER_PORT_REST) --name=$(DOCKER_NAME)-run $(PACKAGE_NAME):latest
	@echo "$(YELLOW)Docker running and listening to http://localhost:$(HOST_PORT_GRPC) and http://localhost:$(HOST_PORT_REST) $(SGR0)"

docker-stop:
	@docker kill ${DOCKER_NAME}-run || true
