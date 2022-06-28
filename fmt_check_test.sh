#! /bin/bash

# cargo fmt && RUSTFLAGS=-Awarnings cargo check && RUSTFLAGS=-Awarnings cargo test
cargo fmt && cargo check && cargo test