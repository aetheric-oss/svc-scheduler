## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/rust-all/Makefile.tftpl

DOCKER_NAME := arrow-svc-scheduler

# Combined targets

IMAGE_NAME   := svc-scheduler
PACKAGE_NAME := $(IMAGE_NAME)
DOCKER_PORT  := 8080
HOST_PORT    := 8002

help: .help-base .help-rust .help-python .help-cspell .help-markdown .help-editorconfig .help-toml .help-docker
build: rust-build docker-build
all: test build release

include .make/docker.mk

release: rust-release
test: rust-test-all cspell-test toml-test python-test md-test-links editorconfig-test
tidy: rust-tidy toml-tidy python-tidy

include .make/base.mk
include .make/cspell.mk
include .make/markdown.mk
include .make/editorconfig.mk
include .make/toml.mk
include .make/rust.mk
include .make/python.mk
