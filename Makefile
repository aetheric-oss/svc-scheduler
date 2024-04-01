## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/Makefile.tftpl

include .make/env.mk
export

help: .help-base .help-rust .help-python .help-cspell .help-markdown .help-editorconfig .help-commitlint .help-toml .help-docker
build: clean docker-build
clean: rust-clean
release: rust-release
publish: rust-publish
test: rust-test-all cspell-test toml-test python-test md-test-links editorconfig-test
tidy: rust-tidy toml-tidy python-tidy editorconfig-tidy
all: clean test build release publish

include .make/docker.mk
include .make/base.mk
include .make/cspell.mk
include .make/markdown.mk
include .make/editorconfig.mk
include .make/commitlint.mk
include .make/toml.mk
include .make/rust.mk
include .make/python.mk
