# Deadlock Formal Model

This example is intentionally a failure-path example.

It models the smallest possible spec that reaches a deadlock after one valid step:

- `Start` transitions to `Stuck`
- `Stuck` has no outgoing transition
- `formal_tests` therefore report a deadlock violation

Use it when you want to see how `nirvash` surfaces a spec-level deadlock in generated
formal tests, not when you want a happy-path runtime example.

Run the summary binary with:

```bash
cargo run --manifest-path examples/deadlock-formal-model/Cargo.toml
```

Expected output:

```text
spec: deadlock_formal
reachable states: 2
deadlock states: 1
deadlock targets:
  1. stuck
```

Run the spec tests with:

```bash
cargo test --manifest-path examples/deadlock-formal-model/Cargo.toml
```

That command is expected to fail.

- top-level failing test:
  `model::__nirvash_generated_tests_deadlockspec::generated_model_checker_accepts_spec`
- nested subprocess failure reported in its output:
  `model::__nirvash_generated_tests_deadlockspec::generated_model_checker_accepts_spec_case`

The failure reason should mention a `Deadlock` counterexample returned by `check_all()`.
