## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/base.mk

SHELL := /bin/bash

BUILD_IMAGE_NAME := ghcr.io/arrow-air/tools/arrow-rust
BUILD_IMAGE_TAG  := latest
DOCKER_PORT      := 8000
HOST_PORT        := 8080

SOURCE_PATH      ?= $(PWD)

# Style templates for console output.
GREEN  := $(shell echo -e `tput setaf 2`)
YELLOW := $(shell echo -e `tput setaf 3`)
CYAN   := $(shell echo -e `tput setaf 6`)
BOLD   := $(shell echo -e `tput bold`)
SMUL   := $(shell echo -e `tput smul`)
SGR0   := $(shell echo -e `tput sgr0`)

# function with a generic template to run docker with the required values
# Accepts $1 = command to run, $2 = additional flags (optional)
docker_run = docker run \
	--name=$(DOCKER_NAME)-$@ \
	--rm \
	--user `id -u`:`id -g` \
	--workdir=/usr/src/app \
	-v "$(SOURCE_PATH)/:/usr/src/app" \
	$(2) \
	-t $(BUILD_IMAGE_NAME):$(BUILD_IMAGE_TAG) \
	$(1)

.SILENT: docker-pull

.help-base:
	@echo ""
	@echo "$(BOLD)$(CYAN)Available targets$(SGR0)"

docker-pull:
	@docker pull -q $(BUILD_IMAGE_NAME):$(BUILD_IMAGE_TAG)
