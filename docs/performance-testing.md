# Performance Testing Strategy

## Goals

1. Detect performance regressions in CI
2. Test realistic workloads (5000+ files)
3. Cross-platform (Windows, Linux, macOS)
4. Reliable - minimize false positives from CPU load variance

## Why Not Wall-Clock Time?

Wall-clock time varies based on:
- Other processes running
- CPU thermal throttling
- CPU frequency scaling
- CI runner load

This makes absolute thresholds unreliable for CI gating.

## Approach: Ratio-Based Scaling Tests

Instead of measuring absolute time, measure **scaling ratios**:

```
T₁ = time to process 1 file
T₅₀₀₀ = time to process 5000 files
ratio = T₅₀₀₀ / T₁
```

### What the Ratio Tells You

| Ratio | Interpretation |
|-------|----------------|
| ~5000 | O(n) linear scaling - healthy |
| ~5500 | O(n) with ~10% fixed overhead - acceptable |
| ~61,000 | O(n log n) - worth investigating |
| ~25,000,000 | O(n²) - regression, fail CI |

### Why Ratios Work

A fast machine: T₁=10ms, T₅₀₀₀=50s → ratio ≈ 5000
A slow machine: T₁=30ms, T₅₀₀₀=150s → ratio ≈ 5000

The ratio is CPU-independent, making it suitable for CI gates.

### What Ratios Catch

- Algorithmic regressions (O(n) → O(n²))
- Memory thrashing at scale
- Hidden quadratic behavior in dependencies

### What Ratios Don't Catch

- Constant factor slowdowns (everything 2x slower but still O(n))
- Absolute performance ("is 50s acceptable?")

## CPU Time vs Wall Time

For additional stability, use CPU time instead of wall time:

```rust
use cpu_time::ProcessTime;

fn measure_cpu<F: FnOnce()>(f: F) -> Duration {
    let start = ProcessTime::now();
    f();
    start.elapsed()
}
```

CPU time measures actual CPU cycles used by the process, ignoring:
- Time waiting for I/O
- Time stolen by other processes

Note: CPU time sums across all cores, so parallel workloads show higher CPU time than wall time.

## Test Scenarios

### Realistic Workload Requirements

Empty files don't test real bottlenecks. Tests need:
- Files with actual AST content
- Queries that match a subset of files
- Full pipeline: glob → parse → query → output

### Scenarios to Cover

| Scenario | Files | Matches | Purpose |
|----------|-------|---------|---------|
| Sparse matches | 5000 | ~50 (1%) | Query selectivity |
| Dense matches | 5000 | ~500 (10%) | Result aggregation |
| No matches | 5000 | 0 | Early exit optimization |
| All match | 5000 | 5000 | Worst case |

### Known Performance Pitfalls

These are scenarios where we've seen performance problems:

1. **Result aggregation** - Combining matched files into single XML then parsing
2. **Large file handling** - Files with 10,000+ lines
3. **Complex queries** - Deeply nested XPath predicates
4. **Many small matches** - Thousands of tiny matches across files

## Test Corpus Options

### Option 1: Real Repository

Clone a large OSS C# project (Roslyn, Orleans, etc.)

**Pros:**
- Truly realistic code patterns
- Real-world file size distribution

**Cons:**
- Large download (~100MB+)
- May change over time
- Slower CI setup

### Option 2: Generated Files

Template-based generation with controllable patterns:

```csharp
// Generated: file_0042.cs
public class Service42 {
    [MaxLength(100)]  // 10% of files have this
    public string Name { get; set; }

    public void Process() {
        _context.Users.ToList();  // 5% have this anti-pattern
    }
}
```

**Pros:**
- Controllable match rates
- Reproducible
- Small to store (just the generator)
- Fast to create

**Cons:**
- May miss real-world edge cases
- Less variety in AST structures

### Option 3: Snapshot Fixtures

Commit a snapshot of 5000 files from a real project as test fixtures.

**Pros:**
- Real code, stable over time
- No download in CI

**Cons:**
- Adds ~50MB to repository
- Static, won't reflect new edge cases

### Recommendation

Use **Option 2 (Generated Files)** for CI, with occasional manual testing against real repositories.

The generator should produce:
- Valid, parseable source files
- Configurable match patterns
- Realistic AST depth and complexity

## Implementation Sketch

```rust
// tests/perf_scaling.rs
use cpu_time::ProcessTime;
use tempfile::TempDir;

fn measure_cpu<F: FnOnce() -> R, R>(f: F) -> (R, Duration) {
    let start = ProcessTime::now();
    let result = f();
    (result, start.elapsed())
}

#[test]
#[ignore]  // Run with: cargo test --ignored
fn parsing_scales_linearly() {
    let corpus = generate_test_corpus(5000);

    let (_, t1) = measure_cpu(|| parse_files(&corpus[..1]));
    let (_, t1000) = measure_cpu(|| parse_files(&corpus[..1000]));
    let (_, t5000) = measure_cpu(|| parse_files(&corpus));

    let ratio_1000 = t1000.as_secs_f64() / t1.as_secs_f64();
    let ratio_5000 = t5000.as_secs_f64() / t1.as_secs_f64();

    assert!(ratio_1000 < 1200.0,
        "1000-file ratio {ratio_1000:.0} exceeds 1200");
    assert!(ratio_5000 < 6000.0,
        "5000-file ratio {ratio_5000:.0} exceeds 6000");
}

#[test]
#[ignore]
fn query_with_sparse_matches() {
    let corpus = generate_test_corpus_with_matches(5000, 0.01); // 1% match

    let (_, t1) = measure_cpu(|| query_files(&corpus[..1], "//method"));
    let (_, t5000) = measure_cpu(|| query_files(&corpus, "//method"));

    let ratio = t5000.as_secs_f64() / t1.as_secs_f64();
    assert!(ratio < 6000.0, "Query ratio {ratio:.0} exceeds 6000");
}
```

## CI Integration

```yaml
# .github/workflows/perf.yml
perf-tests:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo test --release --ignored -- perf_
```

Run performance tests:
- On `main` branch commits
- On PRs with `perf` label
- Nightly for trend tracking

Use `#[ignore]` attribute so normal `cargo test` skips them.

## Future Improvements

1. **Trend tracking** - Store results over time, alert on gradual regression
2. **Flamegraph integration** - Auto-generate flamegraphs when ratio exceeds warning threshold
3. **Real repo testing** - Periodic tests against Roslyn/Orleans in separate workflow
