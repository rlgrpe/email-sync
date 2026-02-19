# AGENTS.md

## Build

```bash
cargo build
cargo build --all-features
```

## Test

```bash
cargo test --all-features -- --show-output
```

## Lint

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Project Structure

```
src/           - Library source code
examples/      - Usage examples
tests/         - Integration tests
```
