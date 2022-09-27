![Arrow Banner](https://github.com/Arrow-air/.github/raw/main/profile/assets/arrow_v2_twitter-banner_neu.png)

# svc-scheduler Service

## :telescope: Overview
**svc-scheduler** is responsible for scheduling, confirming and cancelling flights. 
The service exposes two crates:
 - server - [bin] target to run gRPC server
 - client - [lib] target for other services to import and use

Directory:
- `server/`: Source Code and Unit Tests for scheduler-service server
- `client/`: Source Code for scheduler-service client
- `client/tests/`: Integration Tests
- `docs/`: Module Documentation

## Installation

Install Rust with [Rustup](https://www.rust-lang.org/tools/install).

```bash
# After cloning the repository
python3 -m pip install -r requirements.txt

# Adds custom pre-commit hooks to .git through cargo-husky dependency
# !! Required for developers !!
cargo test
```

## :scroll: Documentation
The following documents are relevant to this service:
- [Concept of Operations](./docs/conops.md)
- [Requirements & User Stories](./docs/requirements.md)
- [SDD](./docs/sdd.md)(TODO)
- [ICD](./docs/icd.md)

## :busts_in_silhouette: Arrow DAO
Learn more about us:
- [Website](https://www.arrowair.com/)
- [Arrow Docs](https://www.arrowair.com/docs/intro)
- [Discord](https://discord.com/invite/arrow)

## :exclamation: Treatment of `Cargo.lock`
If you are building a non-end product like a library, include `Cargo.lock` in `.gitignore`.

If you are building an end product like a command line tool, check `Cargo.lock` to the git. 

Read more about it [here](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html);
