# Lock Manager Model

This example models a tiny lock manager with two clients and one shared lock.

- `Alice` and `Bob` can `Request`, `Grant`, and `Release`
- both clients may wait at the same time
- the model guarantees mutual exclusion
- the binary prints one handoff witness from `Alice` to `Bob`

It is meant to show the `nirvash` happy path for a TLA+-style system in Rust:

- author the transition system with `FrontendSpec + TemporalSpec`
- bind a mock runtime with `#[nirvash_binding]`
- install all runnable generated tests with `generated::install::all_tests!`
- explore the reachable graph with `ExplicitModelChecker`

The source is split by responsibility:

- `src/model.rs`: state, action, invariant, and spec definition
- `src/runtime.rs`: mock runtime and binding implementation
- `src/planning.rs`: lowering, reachable-graph exploration, and witness formatting
- `src/tests.rs`: direct runtime tests and summary assertions
- `src/lib.rs`: public surface and generated-test import
- `src/main.rs`: CLI entrypoint that prints the summary

Run it with:

```bash
cargo run --manifest-path examples/lock-manager-model/Cargo.toml
```

Test the runnable generated suites (`smoke_default`, `boundary_default`, `unit_default`,
`e2e_default`, `concurrency_default`) with:

```bash
cargo test --manifest-path examples/lock-manager-model/Cargo.toml -- --nocapture
```

If your shell already exports `RUST_LOG`, that value wins. To force route logs, run
`RUST_LOG=debug cargo test --manifest-path examples/lock-manager-model/Cargo.toml -- --nocapture`.

Inspect the generated profiles that `cargo nirvash` can see:

```bash
cargo run -p cargo-nirvash -- \
  --base examples/lock-manager-model/target/nirvash \
  list-tests \
  --spec LockManagerSpec
```

`all_tests!` runs every generated profile the binding supports:

- explicit/proptest suites from `smoke_default`, `boundary_default`, and `unit_default`
- trace validation from `e2e_default`
- loom + shuttle from `concurrency_default`

With `-- --nocapture`, generated tests emit `tracing::debug!` route logs. Expect lines like:

```text
DEBUG generated test wrapper start test_name="generated_all_tests" spec="LockManagerSpec" binding="crate::runtime::MockLockManager" profiles=["smoke_default", "boundary_default", "unit_default", "e2e_default", "concurrency_default"]
DEBUG generated action route test_name="generated_all_tests" spec="LockManagerSpec" profile="e2e_default" engine="trace_validation" binding="crate::runtime::MockLockManager" model_case="default" route="Request(Alice)" route_len=1 outcome="ok" case_index=None replay_path="" schedule_path=""
DEBUG generated action route test_name="generated_all_tests" spec="LockManagerSpec" profile="concurrency_default" engine="shuttle_pct" binding="crate::runtime::MockLockManager" model_case="default" route="Request(Alice) -> Grant(Alice)" route_len=2 outcome="ok" case_index=None replay_path="" schedule_path="target/nirvash/replay/src_model_rs__line126_col0_LockManagerSpec_concurrency_default_schedule_json"
DEBUG generated action route test_name="replay_src_model_rs__line126_col0_lockmanagerspec_e2e_default" spec="LockManagerSpec" profile="e2e_default" engine="replay" binding="lock_manager_model::runtime::MockLockManager" model_case="replay" route="Request(Alice)" route_len=1 outcome="ok" case_index=None replay_path="target/nirvash/replay/src_model_rs__line126_col0_LockManagerSpec_e2e_default_trace_validation_bundle_json" schedule_path=""
```

Materialize a replay test from the generated replay bundle after the manifests exist:

```bash
cargo run -p cargo-nirvash -- \
  --base examples/lock-manager-model/target/nirvash \
  materialize-tests \
  --spec LockManagerSpec \
  --binding 'crate :: runtime :: MockLockManager' \
  --profile e2e_default
```

That refreshes `tests/generated.rs`, so the materialized replay can be executed with:

```bash
cargo test --manifest-path examples/lock-manager-model/Cargo.toml --test generated -- --nocapture
```

Measure example-only coverage with runnable generated tests:

```bash
cargo llvm-cov \
  --manifest-path examples/lock-manager-model/Cargo.toml \
  --summary-only \
  --ignore-filename-regex '(^|/)crates/'
```

Expected output:

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
