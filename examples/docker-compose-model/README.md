# Docker Compose Model

This example models a small `docker compose up` style stack with three services:

- `db`
- `cache`
- `web`

The model encodes dependency ordering and health gates:

- `cache` may only be created after `db` is healthy
- `web` may only be created after both `db` and `cache` are healthy
- once every service is healthy, the stack can stay in a steady state

The example also includes a mock runtime implementation and tests:

- plain unit tests that call the mock runtime directly
- generated `nirvash` code tests for explicit state-space coverage
- generated trace-validation tests for the bound runtime
- `nirvash-docgen` output embedded into rustdoc for the spec type

The source is split by responsibility:

- `src/model.rs`: formal model, state/action types, and the spec definition
- `src/runtime.rs`: mock runtime and `#[nirvash_binding]` implementation
- `src/planning.rs`: lowering, reachable-graph exploration, and plan formatting
- `src/tests.rs`: plain unit tests that exercise the mock runtime directly
- `build.rs`: rustdoc fragment generation through `nirvash-docgen`

Run it with:

```bash
cargo run --manifest-path examples/docker-compose-model/Cargo.toml
```

Test it with:

```bash
cargo test --manifest-path examples/docker-compose-model/Cargo.toml
```

Generate docs with:

```bash
RUSTDOCFLAGS='-A rustdoc::invalid_html_tags' \
  cargo doc --manifest-path examples/docker-compose-model/Cargo.toml --no-deps
```

The library exposes the model, a mock `MockComposeRuntime`, and helper functions for planning.
The binary lowers the spec, explores the reachable graph with the explicit checker, verifies
the dependency invariants, and prints one valid bring-up plan.
