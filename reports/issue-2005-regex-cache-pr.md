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

Measured with the existing benchmark path in `reports/regex-cache-bench`:

```text
BENCH_SIZES=1000,100000 MEASURED_RUNS=5 cargo run --release --manifest-path reports/regex-cache-bench/Cargo.toml
```

### Micro

- `Regex::new("ell")`: 2,734.6 ns/op
- `is_match("ell")`: 7.8 ns/op
- `Regex::new("^a\|g")`: 5,282.2 ns/op
- `is_match("^a\|g")`: 11.2 ns/op
- 복합 패턴 컴파일: 5,717.3 ns/op
- case-insensitive 컴파일: 5,712.8 ns/op
- thread-local HashMap lookup: 21.7 ns/op

### End-to-end

- 1,000 rows, regex constant: 2.021 ms
- 1,000 rows, LIKE constant: 2.270 ms
- 1,000 rows, regex dynamic: 7.022 ms
- 100,000 rows, regex constant: 211.275 ms
- 100,000 rows, LIKE constant: 235.551 ms
- 100,000 rows, regex dynamic: 714.360 ms

Conditions: release build, 3 warm-up runs, 5 measured runs, median values, `MemoryStorage`.
Queries: `name ~ '^match-[0-9]+$'`, `name LIKE 'match-%'`, and `name ~ pattern`.

## Validation

- `cargo test -p gluesql-core`: passed, 607 tests and 1 doctest.
- `cargo fmt --all`: passed.
- `git diff --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: blocked by pre-existing warnings in unrelated files:
  - `macros/src/lib.rs`
  - `core/src/executor/evaluate/evaluated/eq.rs`
  - `core/src/planner/expr/plan_expr/function.rs`
