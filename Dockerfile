## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/Dockerfile

FROM --platform=$BUILDPLATFORM ghcr.io/arrow-air/tools/arrow-rust:1.2 AS build

ENV CARGO_INCREMENTAL=1
ENV RUSTC_BOOTSTRAP=0
ARG ENABLE_FEATURES=

COPY . /usr/src/app

# perl and build-base are needed to build openssl, see:
# https://github.com/openssl/openssl/blob/master/INSTALL.md#prerequisites
RUN apk -U add perl build-base
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cd /usr/src/app && \
    cargo build --release --features=vendored-openssl,${ENABLE_FEATURES}

FROM --platform=$TARGETPLATFORM ghcr.io/grpc-ecosystem/grpc-health-probe:v0.4.19 AS grpc-health-probe

FROM --platform=$TARGETPLATFORM alpine:latest
ARG PACKAGE_NAME=
COPY --from=grpc-health-probe /ko-app/grpc-health-probe /usr/local/bin/grpc_health_probe
COPY --from=build /usr/src/app/target/release/${PACKAGE_NAME} /usr/local/bin/${PACKAGE_NAME}
RUN ln -s /usr/local/bin/${PACKAGE_NAME} /usr/local/bin/server

ENTRYPOINT ["/usr/local/bin/server"]
