# Add execution-scoped regex cache

## Related issue

- Closes #2005

## Summary

Adds a bounded regex cache scoped to query execution. Repeated LIKE and regex patterns can reuse compiled expressions without extending cache state beyond the execution scope.

## Changes

- Add a bounded `RegexCache` owned by `ExecutionContext`.
- Keep query execution contexts isolated, including interleaved iterators.
- Create a temporary scope for direct stateless evaluation when no execution scope is active.
- Share LIKE and regex matching logic while preserving the existing evaluation behavior.
- Add cache reuse, cache separation, fallback-scope, iterator-scope, and repeated-query tests.
- Keep unrelated planner, macro, and storage changes out of the implementation.

## Benchmark

Measured with the existing benchmark path in `reports/regex-cache-bench`.

- 1,000 rows: regex constant 2.021 ms, LIKE constant 2.270 ms, regex dynamic 7.022 ms.
- 100,000 rows: regex constant 211.275 ms, LIKE constant 235.551 ms, regex dynamic 714.360 ms.

Detailed measurements are in `reports/regex-cache-bench/RESULTS.md`.

## Validation

- `cargo test -p gluesql-core`: passed, 607 tests and 1 doctest.
- `cargo fmt --all`: passed.
- `git diff --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: blocked by pre-existing warnings in unrelated files:
  - `macros/src/lib.rs`
  - `core/src/executor/evaluate/evaluated/eq.rs`
  - `core/src/planner/expr/plan_expr/function.rs`
