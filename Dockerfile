## DO NOT EDIT!
# This file was provisioned by Terraform
# File origin: https://github.com/Arrow-air/tf-github/tree/main/src/templates/rust-svc/Dockerfile
FROM --platform=$BUILDPLATFORM ghcr.io/arrow-air/tools/arrow-rust:latest AS build

ENV CARGO_INCREMENTAL=1
ENV RUSTC_BOOTSTRAP=0

COPY . /usr/src/app

RUN cd /usr/src/app ; cargo build --release

FROM --platform=$TARGETPLATFORM alpine:latest
ARG PACKAGE_NAME=
COPY --from=build /usr/src/app/target/release/${PACKAGE_NAME} /usr/local/bin/${PACKAGE_NAME}
RUN ln -s /usr/local/bin/${PACKAGE_NAME} /usr/local/bin/server

ENTRYPOINT ["/usr/local/bin/server"]
