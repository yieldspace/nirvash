# nirvash

`nirvash` is a standalone Rust workspace for writing and checking transition-system
specifications in the style of TLA+, but with Rust types, macros, checkers, and runtime
bindings.

It is not a TLA+ parser or a full compatibility layer. The core contract is:

```text
FrontendSpec + TemporalSpec -> LoweredSpec -> checker / conformance / proof / docgen
```

That shared boundary lets one authored system flow into explicit model checking, symbolic
checking, generated runtime tests, proof export, and documentation generation without changing
the spec.

## Five-Minute Tour

The shortest introduction is the lock-manager example:

```bash
cargo run --manifest-path examples/lock-manager-model/Cargo.toml
```

It models a tiny system with two clients and one shared lock:

- `Request(client)` moves a client from `Idle` to `Waiting`
- `Grant(client)` moves a waiting client to `Holding` when nobody else holds the lock
- `Release(client)` returns the holder to `Idle`
- an invariant enforces mutual exclusion

The example binary lowers the Rust-authored spec, explores the full reachable graph, and prints
one witness for a lock handoff from Alice to Bob:

```text
spec: lock_manager
reachable states: 8
holding states: 4
sample handoff plan:
  1. alice requests lock
  2. alice granted lock
  3. bob requests lock
  4. alice releases lock
  5. bob granted lock
target state: LockState { alice: Idle, bob: Holding }
```

To verify the mock runtime against the same model:

```bash
cargo test --manifest-path examples/lock-manager-model/Cargo.toml -- --nocapture
```

If your shell already exports `RUST_LOG`, that value wins. Use `RUST_LOG=debug` to force
the generated route logs on.

That example includes:

- a `FrontendSpec + TemporalSpec` model
- a `#[nirvash_binding]` mock runtime
- generated tests installed through `generated::install::all_tests!`
- `tracing::debug!` route logs for generated test execution with `-- --nocapture`
- reachable-graph exploration through `ExplicitModelChecker`

## How The Workspace Fits Together

- `crates/nirvash`
  - Rust-first authoring facade with DSL entry points such as `pred!`, `step!`, `ltl!`,
    `TransitionProgram`, and DocGraph helpers
- `crates/nirvash-lower`
  - Canonical lowering boundary centered on `LoweredSpec`
- `crates/nirvash-check`
  - Stable explicit and symbolic checker front doors
- `crates/nirvash-conformance`
  - Runtime replay, generated harness plans, and adapters for generated tests
- `crates/nirvash-proof`
  - Proof bundle export and certificate-oriented types
- `crates/nirvash-docgen`
  - Rustdoc-oriented doc graph and Mermaid generation helpers
- `crates/nirvash-macros`
  - Proc macros for derives, subsystem specs, runtime bindings, and generated tests
- `crates/cargo-nirvash`
  - `cargo nirvash` subcommand implementation

For the authoring facade and the full architecture diagrams, see
[`crates/nirvash/README.md`](crates/nirvash/README.md).

## Examples

- `examples/lock-manager-model`
  - Smallest end-to-end example for TLA+-style system modeling in Rust
- `examples/docker-compose-model`
  - Larger example that adds docgen output, a richer runtime, and a more operational state
    machine

## Tooling

- `cargo nirvash list-tests`
- `cargo nirvash materialize-tests`
- `cargo nirvash replay`

Generated artifacts are written under `target/nirvash/{manifest,replay}`. Materialized
replay files are written to `tests/generated/*.rs`, and `tests/generated.rs` is refreshed
so Cargo can run them as an integration test crate.

## Development

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

For the first standalone `0.1.0` line, unpublished sibling crates are still resolved through a
temporary local `[patch.crates-io]` overlay during packaging. The CI workflow generates that
overlay before running the workspace `cargo package` step.

Optional engines such as `loom` and `shuttle` remain opt-in. The default workspace
checks do not require them.

## License

Apache-2.0
