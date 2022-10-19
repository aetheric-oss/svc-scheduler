## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/rust-all/Makefile.tftpl

DOCKER_NAME          := arrow-svc-scheduler
PACKAGE_NAME         := svc-scheduler

PUBLISH_PACKAGE_NAME := svc-scheduler-client-grpc
DOCKER_PORT_REST     := 8000
DOCKER_PORT_GRPC     := 50051
HOST_PORT_REST       := 8002
HOST_PORT_GRPC       := 50002

help: .help-base .help-rust .help-python .help-cspell .help-markdown .help-editorconfig .help-commitlint .help-toml .help-docker
build: rust-build docker-build

include .make/docker.mk

export

clean: rust-clean
release: rust-release
publish: rust-publish
test: rust-test-all cspell-test toml-test python-test md-test-links editorconfig-test
tidy: rust-tidy toml-tidy python-tidy editorconfig-tidy
all: clean test build release publish

include .make/base.mk
include .make/cspell.mk
include .make/markdown.mk
include .make/editorconfig.mk
include .make/commitlint.mk
include .make/toml.mk
include .make/rust.mk
include .make/python.mk
