# nirvash

`nirvash` is a standalone Rust workspace for authoring, lowering, checking, replaying,
and documenting executable transition-system specifications.

The repository keeps the formal-spec tooling, checker front doors, proof export surface,
generated code-test harnesses, and the `cargo nirvash` CLI in one publishable workspace.

## Workspace Layout

- `crates/nirvash`
  - Authoring facade crate with DSL entry points such as `pred!`, `step!`, `ltl!`,
    `TransitionProgram`, and DocGraph helpers.
- `crates/nirvash-foundation`
  - Shared finite-domain and symbolic-encoding traits.
- `crates/nirvash-ir`
  - Backend-neutral lowered core and proof obligation types.
- `crates/nirvash-lower`
  - Lowering boundary and checker-facing shared API centered on `LoweredSpec`.
- `crates/nirvash-check`
  - Explicit and symbolic checker front doors.
- `crates/nirvash-backends`
  - Explicit and SMT-backed backend implementations.
- `crates/nirvash-conformance`
  - Runtime replay, generated harness plans, and test adapters.
- `crates/nirvash-proof`
  - Proof bundle export and certificate-oriented types.
- `crates/nirvash-docgen`
  - Rustdoc-oriented doc graph and Mermaid generation helpers.
- `crates/nirvash-macros`
  - Proc macros for derive support, registry wiring, subsystem specs, and generated tests.
- `crates/cargo-nirvash`
  - `cargo nirvash` subcommand implementation.

## Core Boundary

`nirvash` keeps one shared execution boundary:

- Author specs against `FrontendSpec`, `TemporalSpec`, and the DSL surface in `nirvash`.
- Lower authored semantics through `nirvash-lower` into `LoweredSpec`.
- Feed `LoweredSpec` into the checker, conformance, proof, and documentation crates.

This keeps authoring concerns separate from execution backends while preserving one
canonical lowered representation for explicit checking, symbolic checking, replay, and proof
export.

## Tooling

- `cargo nirvash list-tests`
- `cargo nirvash materialize-tests`
- `cargo nirvash replay`

Generated artifacts are written under `target/nirvash/{manifest,replay}` and materialized
test files are written to `tests/generated`.

## Development

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

For the first standalone `0.1.0` line, unpublished sibling crates are still resolved through a
temporary local `[patch.crates-io]` overlay during packaging. The CI workflow generates that
overlay before running the workspace `cargo package` step.

Optional engines such as `kani`, `loom`, and `shuttle` remain opt-in. The default workspace
checks do not require them.

## License

Apache-2.0
