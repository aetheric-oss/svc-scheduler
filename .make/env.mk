## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/all/.make/env.mk

ENV_FILE := $(wildcard .env .env.base)
# If .env is missing, install .env.base+.env.repo as .env and load that.
ifeq ($(ENV_FILE),.env.base)
$(warning Settings file '.env' missing: '$(ENV_FILE)' => installing .env as merge of .env.base + .env.repo!)
$(shell cat .env.base .env.repo > .env 2>/dev/null)
endif

# Sanity check, make sure keys from base and repo are present.
# Strip empty lines and comments, get sorted keys (e.g. DOCKER_NAME)
ENV_KEYS=$(shell grep -Ehv '^\s*(\#.*)?\s*$$' .env.base .env.repo 2>/dev/null | cut -f1 -d= | sort)
# Check if key exists in .env, if not grep it from the .env.base/repo into your .env
$(foreach k, $(ENV_KEYS), $(eval $(shell sh -c "grep -q '^$(k)=' .env 2>/dev/null || (echo '*** NOTE: Adding missing .env key [$(k)] to your .env file!' 1>&2 ; grep -h '^$(k)=' .env.base .env.repo 2>/dev/null >> .env ; grep '^$(k)=' .env 1>&2)")))

-include $(ENV_FILE)
