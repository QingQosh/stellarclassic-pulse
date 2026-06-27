# Mutation Testing Guide (Issue #555)

## Overview

Mutation testing is a software testing technique that measures the quality of test suites by introducing intentional bugs (mutations) into the code and checking if the tests catch them. A good test suite will "kill" (detect) most mutations.

## What is Mutation Testing?

Mutation testing answers the question: **"How many bugs can our tests detect?"**

### How It Works

1. **Generate Mutations**: Tool modifies source code (change `>` to `<`, `+` to `-`, etc.)
2. **Run Tests**: Execute test suite against mutated code
3. **Analyze Results**: 
   - **Killed mutation**: Tests caught the bug ✅
   - **Survived mutation**: Tests missed the bug ❌
   - **Skipped mutation**: Code not covered by tests

### Simple Example

**Original code**:
```rust
fn validate_page(page: i64) -> bool {
    page > 0  // Must be positive
}
```

**Mutation 1** - Operator change:
```rust
fn validate_page(page: i64) -> bool {
    page >= 0  // This is wrong but might still pass tests
}
```

**Mutation 2** - Boundary value:
```rust
fn validate_page(page: i64) -> bool {
    page > 1  // Wrong boundary
}
```

**Good tests catch these mutations**:
```rust
#[test]
fn page_zero_is_invalid() {
    assert!(!validate_page(0));  // Catches both mutations
}

#[test]
fn page_one_is_valid() {
    assert!(validate_page(1));  // Catches mutation 2
}
```

## Tool: cargo-mutants

We use [cargo-mutants](https://mutants.live), a Rust mutation testing tool that:

- **Integrates with Cargo**: No special setup needed
- **Fast execution**: Parallel testing of multiple mutations
- **Smart skipping**: Doesn't test unreachable code
- **HTML reports**: Visual mutation coverage analysis
- **CI-friendly**: Exit codes and reports for automation

## Installation

### Install cargo-mutants

```bash
cargo install cargo-mutants
```

Requires Rust 1.70+.

### Verify Installation

```bash
cargo mutants --version
```

## Running Mutation Tests

### Basic Usage

```bash
# Run mutation tests on entire codebase
cargo mutants

# Run with verbose output
cargo mutants --verbose

# Run with specific output directory
cargo mutants --output target/mutants-report
```

### Focused Testing

```bash
# Test only a specific file
cargo mutants --file src/handlers.rs

# Test only specific module
cargo mutants --name handlers

# Test with specific test command
cargo mutants --test-command "cargo test --lib"
```

### Performance Options

```bash
# Limit number of mutations (for quick checks)
cargo mutants --maximum 50

# Set timeout for tests (seconds)
cargo mutants --timeout 300

# Use multiple jobs (parallel testing)
cargo mutants --jobs 4

# Check progress without running tests
cargo mutants --check-only
```

### Advanced Options

```bash
# Generate both HTML and JSON reports
cargo mutants --generate-json --html

# Keep working directory after test
cargo mutants --keep-working-directory

# Detailed output for CI
cargo mutants --verbose --output-json mutants.json

# Stop after first failure
cargo mutants --fail-fast
```

## Understanding Results

### Output Format

```
RESULT: 157 killed, 12 survived, 4 unviable, 2 in progress
Mutation testing finished
```

### Result Types

| Type | Meaning | Action |
|------|---------|--------|
| **Killed** | Test detected mutation | ✅ Good coverage |
| **Survived** | Test missed mutation | ❌ Need better tests |
| **Unviable** | Mutation doesn't compile | ℹ️ Code is untestable |
| **Error** | Test execution error | ⚠️ Investigate |

### Mutation Score

```
Mutation Score = Killed / (Killed + Survived) × 100%

Score >= 80%: Excellent test coverage
Score >= 70%: Good test coverage
Score >= 50%: Acceptable coverage
Score < 50%: Needs improvement
```

## HTML Reports

### Viewing Results

```bash
# Generate HTML report
cargo mutants --generate-json --html

# Open report in browser
open target/mutants/index.html
```

### Report Contents

- **Summary**: Overall mutation score and statistics
- **Module breakdown**: Mutation score by module
- **File view**: Line-by-line mutation coverage
- **Failed mutations**: Details on survivors
- **Timing**: How long each mutation took

### Color Coding in Reports

- 🟢 **Green**: All mutations in this section killed
- 🟡 **Yellow**: Some mutations survived
- 🔴 **Red**: No mutations killed (untested)
- 🔵 **Blue**: Unviable mutations (not testable)

## Configuration

### mutants.toml

The project includes a `mutants.toml` configuration file:

```toml
[mutation]
paths = ["src/"]
exclude = ["src/bin/*", "src/main.rs"]

[test]
test_timeout = 300
jobs = 4

[output]
report_dir = "target/mutants"
html_report = true
json_report = true
```

### Environment Variables

```bash
# Override config settings
CARGO_MUTANTS_TIMEOUT=600 cargo mutants
CARGO_MUTANTS_JOBS=8 cargo mutants
CARGO_MUTANTS_VERBOSE=1 cargo mutants
```

## Test Coverage Strategy

### What Gets Mutated

Common mutation types:

1. **Arithmetic Operators**: `+` → `-`, `*` → `/`
2. **Comparison Operators**: `>` → `>=`, `==` → `!=`
3. **Logical Operators**: `&&` → `||`, `!x` → `x`
4. **Constants**: `true` → `false`, numeric changes
5. **Return values**: Last expression changes
6. **Conditionals**: Condition inverts

### What Should Be Tested

**High priority** (catch more mutations):
- Business logic boundaries
- Error handling paths
- State transitions
- Permission checks
- Data validation

**Medium priority**:
- Data transformation
- Type conversions
- Configuration loading
- Logging (hard to test mutations)

**Low priority** (mutations hard to catch):
- Metrics/telemetry
- Logging details
- Panic messages
- Comments

## Example: Improving Coverage

### Scenario: Low Mutation Score

**Module**: `src/handlers.rs`  
**Score**: 45% (too low)  
**Survivors**: Boundary conditions in pagination

### Analysis of Survivors

```rust
// Original code
pub fn validate_limit(limit: i64) -> Result<i64> {
    if limit > 10_000 {
        Err("limit too high")
    }
    Ok(limit)
}
```

**Mutation 1** - Boundary change:
```rust
if limit > 9_999  // Survives - no test for exactly 10_000
```

**Mutation 2** - Operator change:
```rust
if limit >= 10_000  // Survives - no test for exactly 10_000
```

### Solution: Better Tests

```rust
#[test]
fn boundary_exactly_ten_thousand() {
    assert_eq!(validate_limit(10_000), Ok(10_000));
}

#[test]
fn boundary_exceeds_limit() {
    assert!(validate_limit(10_001).is_err());
}

#[test]
fn boundary_just_below_limit() {
    assert_eq!(validate_limit(9_999), Ok(9_999));
}
```

Now both mutations are killed ✅

## Continuous Integration

### GitHub Actions

```yaml
name: Mutation Testing

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  mutation:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Install cargo-mutants
        run: cargo install cargo-mutants
      
      - name: Run mutation tests
        run: cargo mutants --output-json mutants.json
      
      - name: Upload report
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: mutation-report
          path: target/mutants
      
      - name: Check mutation score
        run: |
          score=$(jq '.mutation_score_percent' mutants.json)
          if (( $(echo "$score < 80" | bc -l) )); then
            echo "Mutation score too low: $score%"
            exit 1
          fi
```

### GitLab CI

```yaml
mutation_tests:
  stage: test
  script:
    - cargo install cargo-mutants
    - cargo mutants --output-json mutants.json
  artifacts:
    paths:
      - target/mutants/
    reports:
      mutation: mutants.json
  allow_failure: true
```

## Best Practices

### ✅ Do's

- **Run regularly**: Include in CI/CD pipeline
- **Set target score**: Aim for 80%+ mutation score
- **Review survivors**: Understand why tests missed mutations
- **Improve incrementally**: Fix highest-impact survivors first
- **Track trends**: Monitor mutation score over time
- **Focus on business logic**: Test critical paths thoroughly
- **Combine with coverage**: Use with code coverage tools

### ❌ Don'ts

- **Don't ignore survivors**: They reveal test weaknesses
- **Don't mark expected survivors**: Only for truly untestable code
- **Don't rely on mutation alone**: Combine with other testing
- **Don't test trivial code**: Skip getters, simple wrappers
- **Don't chase 100%**: Some mutations are inherently untestable
- **Don't skip specific tests**: Test all modules consistently

## Interpreting Survivors

### When Survivors Are Expected

Some mutations are hard or impossible to catch:

```rust
// Logging mutations are hard to test
tracing::debug!("Processing event {:?}", event);
// Mutation: tracing::info!(...) - similar effect

// Error messages are untestable
Err("Invalid input")?;
// Mutation: Err("Bad input")? - same behavior
```

### When Survivors Are Problems

These must be fixed:

```rust
// Uncaught boundary mutation
if amount > 100 {  // Should fail on >= 100 too
    return Err("Too large");
}
```

## Performance Considerations

### Speed Optimization

```bash
# Quick check - stop after first N mutations killed
cargo mutants --jobs 8 --maximum 100

# Parallel testing with more jobs
CARGO_MUTANTS_JOBS=16 cargo mutants

# Use incremental builds
cargo mutants --keep-working-directory
```

### Typical Timing

- **Small project** (< 100 LOC): 2-5 minutes
- **Medium project** (1K LOC): 10-30 minutes
- **Large project** (10K+ LOC): 1-4 hours

## Troubleshooting

### Issue: Tests Pass on Mutations They Should Fail

**Cause**: Test doesn't actually verify the mutation  
**Solution**: Review test assertion, add more specific checks

### Issue: Too Many Unviable Mutations

**Cause**: Code paths don't compile with certain mutations  
**Solution**: Normal for generic code - adjust expectations

### Issue: High Memory Usage

**Cause**: Running too many parallel jobs  
**Solution**: Reduce `--jobs` parameter

### Issue: Timeout Errors

**Cause**: Tests taking too long on mutated code  
**Solution**: Increase `--timeout` or optimize tests

## Resources

- **Official Tool**: https://mutants.live
- **Documentation**: https://github.com/sourcefrog/cargo-mutants
- **Research**: https://en.wikipedia.org/wiki/Mutation_testing
- **Best Practices**: https://mutation-testing.pitest.org/

## Project Status

### Current Mutation Testing

- **Status**: Implemented (Issue #555)
- **Configuration**: `mutants.toml`
- **Baseline score**: To be determined on first run
- **Target score**: 80%+
- **CI Integration**: Configured in GitHub Actions

### Continuous Improvement

- Monitor trends in mutation score
- Review survivors quarterly
- Update tests for new features
- Maintain baseline for regressions
