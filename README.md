![Arrow Banner](https://github.com/Arrow-air/.github/raw/main/profile/assets/arrow_v2_twitter-banner_neu.png)

# `svc-scheduler`

![Rust
Checks](https://github.com/arrow-air/svc-scheduler/actions/workflows/rust_ci.yml/badge.svg?branch=main)
![Python Flake8](https://github.com/arrow-air/svc-scheduler/actions/workflows/python_ci.yml/badge.svg?branch=main)
![Arrow DAO
Discord](https://img.shields.io/discord/853833144037277726?style=plastic)


## :telescope: Overview
**svc-scheduler** is responsible for scheduling, confirming and cancelling flights. 
The service exposes two crates:
- server - [bin] target to run gRPC server
- client - [lib] target for other services to import and use

Directory:
- `server/`: Source Code and Unit Tests for scheduler-service server
- `client-grpc/`: Source Code for scheduler-service client
- `proto/`: Types used for gRPC messaging
- `client/tests/`: Integration Tests
- `docs/`: Module Documentation

# Make

### Build and Test

To ensure consistent build and test outputs, Arrow provides a Docker image with all required software installed to build and test Rust projects.
Using the Makefile, you can easily test and build your code.

```bash
# Build Locally
make rust-build

# Create Deployment Container
make build
make docker-run

# If a server is already running
make rust-example-grpc

make docker-stop
```

### Formatting

The Arrow docker image has some formatting tools installed which can fix your code formatting for you.
Using the Makefile, you can easily run the formatters on your code.
Make sure to commit your code before running these commands, as they might not always result in a desired outcome.

```bash
# Format TOML files
make toml-tidy

# Format Rust files
make rust-tidy

# Format Python files
make python-tidy

# Format all at once
make tidy
```

### Other `make` Targets

There are additional make targets available. You can find all possible targets by running make without a target or use `make help`


## :scroll: Documentation
The following documents are relevant to this service:
- [Concept of Operations](./docs/conops.md)
- [SDD](./docs/sdd.md)
- [ICD](./docs/icd.md)
- [Requirements & User Stories](./docs/requirements.md)

## :busts_in_silhouette: Arrow DAO
Learn more about us:
- [Website](https://www.arrowair.com/)
- [Arrow Docs](https://www.arrowair.com/docs/intro)
- [Discord](https://discord.com/invite/arrow)

## :exclamation: Treatment of `Cargo.lock`
If you are building a non-end product like a library, include `Cargo.lock` in `.gitignore`.

If you are building an end product like a command line tool, check `Cargo.lock` to the git.

Read more about it [here](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).
