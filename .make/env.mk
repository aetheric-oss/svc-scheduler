## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/all/.make/env.mk

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

# Add docker user / group ids
$(eval $(shell sh -c "grep -q '^DOCKER_USER_ID=' .env 2>/dev/null || (echo '*** NOTE: Adding missing .env key [DOCKER_USER_ID] to your .env file!' 1>&2 ; echo DOCKER_USER_ID="`id -u`" >> .env ; grep '^DOCKER_USER_ID=' .env 1>&2)"))
$(eval $(shell sh -c "grep -q '^DOCKER_GROUP_ID=' .env 2>/dev/null || (echo '*** NOTE: Adding missing .env key [DOCKER_GROUP_ID] to your .env file!' 1>&2 ; echo DOCKER_GROUP_ID="`id -g`" >> .env ; grep '^DOCKER_GROUP_ID=' .env 1>&2)"))

-include $(ENV_FILE)
