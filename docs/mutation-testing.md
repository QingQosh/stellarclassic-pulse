# Mutation Testing Guide

## Overview

**Mutation testing** is a quality assurance technique that evaluates the effectiveness of your test suite. It works by:

1. Making small changes (mutations) to your source code
2. Running your tests against the mutated code
3. Checking if tests catch (kill) the mutations

If a test suite fails to catch a mutation, it indicates **weak test coverage** for that code path.

### Why Mutation Testing?

Traditional code coverage (line coverage, branch coverage) only measures if code is **executed**, not whether it's **correctly validated** by tests:

```rust
fn abs(x: i32) -> i32 {
    if x < 0 { -x } else { x }
}

#[test]
fn test_abs() {
    assert_eq!(abs(5), 5);  // Tests positive path
}
// This has 100% line coverage but doesn't test x < 0 logic!
```

Mutation testing catches this by mutating the condition:
```rust
// Mutation 1: if x > 0  // SURVIVES (not killed) → test is weak
// Mutation 2: if x <= 0  // SURVIVES → test is weak
```

## Quick Start

### Install cargo-mutants

```bash
cargo install cargo-mutants
```

### Run basic mutation tests

```bash
# Run mutation tests on the entire codebase
cargo mutants --tests

# Run with verbose output (shows each mutation)
cargo mutants --tests -v

# Run in parallel with multiple jobs
cargo mutants --tests --jobs 4
```

### View results

```bash
# JSON report
cargo mutants --tests --output mutations.json

# HTML report
cargo mutants --tests --output mutations.html
```

## Understanding Mutation Results

### Mutation Status

- **Caught** ✅: Test suite detected the mutation (good)
- **Survived** ❌: Mutation was not detected by tests (bad - weak coverage)
- **Unviable** ⚠️: Mutation made code uncompilable
- **Timeout** ⏱️: Mutation tests took too long
- **Missed** ⚠️: cargo-mutants couldn't test this mutation

### Example Output

```
src/models.rs:253: ... (line 253 in models.rs)
  mutation 1:
    Caught: changed limit() > clamp(1, 100) to >
  mutation 2:
    Survived: changed `page.unwrap_or(1)` to `page.unwrap_or(2)`
    ^ Weak test: doesn't verify default value
```

## Using Makefile.mutations

The project includes `Makefile.mutations` for convenient mutation testing:

```bash
# Install cargo-mutants
make -f Makefile.mutations mutants-install

# Run mutation tests
make -f Makefile.mutations mutants

# Run with verbose output
make -f Makefile.mutations mutants-verbose

# Generate HTML report
make -f Makefile.mutations mutants-report

# Run only on library code
make -f Makefile.mutations mutants-lib

# CI-optimized run (uses all cores)
make -f Makefile.mutations mutants-ci
```

## Interpreting Mutation Reports

### JSON Report Structure

```json
{
  "file": "src/models.rs",
  "line": 253,
  "function": "limit",
  "mutation_type": "BinaryOp",
  "original": ">",
  "mutated": ">=",
  "status": "Caught",
  "test_output": "assertion failed in test_limit_bounds"
}
```

### HTML Report

Open `mutations.html` in a browser for interactive visualization:
- Color-coded mutations (green=caught, red=survived)
- Per-file statistics
- Survival statistics by mutation type

## Common Mutation Types

### Arithmetic Operators
- `+` → `-` | `*` | `/` | `%`
- `-` → `+` | `*` | `/` | `%`
- `*` → `/` | `%` | `+` | `-`

### Logical Operators
- `&&` → `||`
- `||` → `&&`
- `!` → removed

### Comparison Operators
- `==` → `!=` | `<` | `<=` | `>` | `>=`
- `<` → `<=` | `>`
- `<=` → `<` | `>`

### Constant Mutations
- `0` → `1` | `-1`
- `1` → `0` | `2`
- `true` → `false`
- `false` → `true`

## Strengthening Tests Based on Mutation Results

### Example: Weak Pagination Tests

**Original test:**
```rust
#[test]
fn test_limit() {
    let params = PaginationParams { page: None, limit: Some(50) };
    assert_eq!(params.limit(), 50);
}
```

**Problem:** This survives mutations like `clamp(1, 100)` → `clamp(1, 99)`

**Strengthened test:**
```rust
#[test]
fn test_limit_bounds() {
    // Test exact boundary values
    assert_eq!(PaginationParams { page: None, limit: Some(1) }.limit(), 1);
    assert_eq!(PaginationParams { page: None, limit: Some(100) }.limit(), 100);
    
    // Test clamping behavior
    assert_eq!(PaginationParams { page: None, limit: Some(0) }.limit(), 1);
    assert_eq!(PaginationParams { page: None, limit: Some(101) }.limit(), 100);
    assert_eq!(PaginationParams { page: None, limit: Some(-1) }.limit(), 1);
    
    // Test edge cases
    assert_eq!(PaginationParams { page: None, limit: None }.limit(), 20);
}
```

Or use **property-based tests** for comprehensive coverage:
```rust
proptest! {
    #[test]
    fn prop_limit_always_in_range(limit in limit_strategy()) {
        let params = PaginationParams { page: None, limit };
        let resolved = params.limit();
        assert!(resolved >= 1 && resolved <= 100);
    }
}
```

## Best Practices

### 1. Run Mutations Regularly
- **Local:** Before pushing (quick version)
- **CI:** Full suite on main/develop branches
- **Scheduled:** Nightly runs to catch regressions

### 2. Target High-Risk Code
```bash
# Run mutations only on core logic
cargo mutants --tests -p soroban-pulse -- src/models.rs
cargo mutants --tests -p soroban-pulse -- src/handlers.rs
```

### 3. Set Reasonable Timeouts
```bash
# Default timeout per test is 30 seconds
cargo mutants --tests --timeout 30

# For slow tests, increase timeout
cargo mutants --tests --timeout 60
```

### 4. Understand Survivorship

Not all surviving mutations indicate weak tests:

**Expected survivors** (safe to ignore):
```rust
// Redundant code (dead)
if x > 0 { }
else if x >= 0 { }  // This branch never executes

// Impossible mutations (unviable)
let arr = [1, 2, 3];
arr[1]  // Mutation: arr[0] → Same index, compiler rejects
```

**Unexpected survivors** (investigate):
```rust
// Weak test: doesn't validate the returned value
fn get_name(&self) -> String {
    self.name.clone()
}

#[test]
fn test_get_name() {
    let obj = Object::new();
    let _ = obj.get_name();  // MUTATION SURVIVES!
}

// Fix: Actually assert the value
#[test]
fn test_get_name() {
    let obj = Object::new();
    assert_eq!(obj.get_name(), "expected");
}
```

### 5. Use Mutation-Driven Testing (MDT)

MDT is a development practice inspired by Test-Driven Development (TDD):

1. **Write production code**
2. **Run mutations** → Identify weak spots
3. **Write tests** to kill mutations
4. **Refactor** code and tests

```bash
# Flow
cargo mutants --tests --output mutations.json
# Review survived mutations
cargo test  # Write new tests
cargo mutants --tests  # Verify improvements
```

## CI Integration

The project uses `.github/workflows/mutation-testing.yml` for automated mutation testing:

```yaml
# Runs on:
# - Every push to main/develop
# - Every PR to main/develop
# - Nightly at 2 AM UTC

# Generates:
# - Mutation report (JSON + HTML)
# - PR comment with summary statistics
# - Fails CI if coverage drops below 80%
```

### Viewing CI Results

1. **GitHub Actions:** View raw output in workflow logs
2. **Artifacts:** Download `mutations.json` and `mutations.html`
3. **PR Comments:** See summary stats in pull request

## Performance Tuning

### Parallelize Tests
```bash
# Use all cores
cargo mutants --tests --jobs $(nproc)

# Use specific number of cores
cargo mutants --tests --jobs 4
```

### Skip Slow Tests
```bash
# Skip tests matching a pattern
cargo mutants --tests --skip-regex 'slow|integration'

# Run only fast tests
cargo mutants --tests -p soroban-pulse
```

### Cache Build Artifacts
```bash
# Use incremental compilation
cargo mutants --tests --incremental

# Leverage cargo cache
CARGO_INCREMENTAL=1 cargo mutants --tests
```

## Troubleshooting

### "Mutation testing takes too long"

**Cause:** Running all mutations on large codebases

**Solutions:**
```bash
# Run on specific module
cargo mutants --tests -- src/models.rs

# Use timeout
cargo mutants --tests --timeout 20

# Parallelize
cargo mutants --tests --jobs 8
```

### "Many mutations marked as unviable"

**Cause:** Code is tightly coupled or impossible to mutate

**Solutions:**
- Review the mutation types being tested
- Check if code is actually executable
- Use `--skip-regex` to skip impossible mutations

### "Tests timeout under mutation"

**Cause:** Mutations slow down code (e.g., changing `<` to `<=`)

**Solutions:**
```bash
# Increase timeout
cargo mutants --tests --timeout 60

# Run selectively
cargo mutants --tests -p soroban-pulse -- src/critical.rs
```

## Resources

- [cargo-mutants GitHub](https://github.com/sourcefrog/cargo-mutants)
- [Mutation Testing (Wikipedia)](https://en.wikipedia.org/wiki/Mutation_testing)
- [Testing Effectiveness (StackOverflow)](https://stackoverflow.com/questions/11656957/mutation-testing)

## Examples in This Project

See these files for examples of mutation-resistant tests:

- `tests/property_tests.rs` - Property-based tests that catch mutations
- `src/models.rs` - Boundaries tests in `#[cfg(test)]` modules
- `tests/search_and_timestamp_tests.rs` - Edge case tests that survive mutations
