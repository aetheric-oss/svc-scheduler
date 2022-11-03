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

## Release R1 demo notes:
1. To run the scheduler service, just run `cargo run`
2. Scheduler requires running svc-storage instance to work properly and by default expects it to be running on `localhost:50052`. 
If you need to change the port, you will need to do it in `main.js` file.
3. The easiest is to run storage service with commented out these 2 lines in main.rs:
```rust
    let pool = PostgresPool::from_config()?;
    pool.readiness().await?;
```
4. When svc-storage runs, it will print 50 vertiport ids which can be used for testing.
5. Run `cargo run --example=client` and see `client/examples/client.rs` for examples of how to use the client. (You will need to change vertiport IDs to ones that are printed by storage service)
6. Standard run should result in positive scenario, changing time to 18:00, should result in error that Departure vertiport is not available.
