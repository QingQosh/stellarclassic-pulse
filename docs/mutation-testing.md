# Mutation Testing Guide

Mutation testing is a quality assurance technique that automatically introduces small bugs (mutations) into the code and verifies that the test suite catches them. This document describes SorobanPulse's mutation testing strategy.

## Overview

Mutation testing answers the question: **"How good are our tests?"**

Rather than measuring code coverage (which only measures what code runs, not whether tests verify correctness), mutation testing measures whether tests would fail if the code were slightly broken.

### Example

Original code:
```rust
pub fn calculate_offset(page: i64, limit: i64) -> i64 {
    (page - 1) * limit
}
```

Mutation 1: Remove the subtraction
```rust
pub fn calculate_offset(page: i64, limit: i64) -> i64 {
    page * limit  // mutation: removed - 1
}
```

Mutation 2: Change the operator
```rust
pub fn calculate_offset(page: i64, limit: i64) -> i64 {
    (page + 1) * limit  // mutation: - changed to +
}
```

Mutation 3: Return a constant
```rust
pub fn calculate_offset(page: i64, limit: i64) -> i64 {
    0  // mutation: returned constant
}
```

A good test suite should **catch all three mutations**. If a test doesn't catch a mutation, that mutation "survives" — indicating a gap in test coverage.

## Why Mutation Testing?

### Code Coverage is Insufficient

Code coverage measures **line execution**, not **correctness**:

```rust
pub fn is_valid_limit(limit: i64) -> bool {
    limit >= 1 && limit <= 100
}

#[test]
fn test_valid_limit() {
    // This test executes all lines but doesn't check correctness
    is_valid_limit(50);  // tests the true branch only
}
```

With 100% line coverage, we might still miss bugs:
- Lower bound mutation: `limit >= 0` (should fail but passes with test above)
- Upper bound mutation: `limit <= 101` (should fail but passes)
- Wrong operator: `limit > 1` (should fail but passes)

Mutation testing catches these.

### Test Quality Metrics

```
Coverage     Mutation Kill Rate    Quality Assessment
100%         95%+                  Excellent
100%         80-95%                Good
100%         60-80%                Fair (missing edge cases)
100%         <60%                  Poor (tests are ineffective)
```

SorobanPulse targets **90%+ mutation kill rate** on core business logic.

## Installing cargo-mutants

```bash
# Install the mutation testing tool
cargo install cargo-mutants

# Verify installation
cargo mutants --version
```

## Running Mutation Tests

### Quick scan (fast, not exhaustive)
```bash
# Run mutations on a single module
cargo mutants -p soroban-pulse -m src/models.rs --shuffle

# Use only 4 threads (faster feedback)
cargo mutants -j 4

# Show detailed mutation output
cargo mutants -v
```

### Full mutation suite (takes longer)
```bash
# Run all mutations with detailed output
cargo mutants

# Run specific module comprehensively
cargo mutants -m src/indexer.rs

# Generate HTML report
cargo mutants --baseline=all --output-dir=target/mutation-report
```

### Check mutation report
```bash
# View the generated HTML report
open target/mutants/index.html  # macOS
xdg-open target/mutants/index.html  # Linux
```

## Understanding Results

Each mutation is classified as:

### ✅ Caught (Good)
The test suite detected the mutation and failed. This is what we want.

```
Mutation: Replace - with +
File: src/models.rs:255
if (page - 1) * limit {
   ~~~~~~~~~~~~^
   Replace with + 

Status: Caught ✓
Test: test_pagination_offset_zero fails
```

### ❌ Survived (Problem)
The test suite didn't catch the mutation. This indicates a test gap.

```
Mutation: Constant assignment
File: src/handlers.rs:412
let offset = (page - 1) * limit;
    ^^^^^^ 
    Always return 0

Status: Survived ✗ (Test gap!)
Recommendation: Add test that verifies different pages produce different offsets
```

### ⊘ Missed (Not evaluated)
The test suite timed out or crashed while testing this mutation. Usually indicates:
- Infinite loop mutation
- Out-of-memory mutation
- Test timeout

```
Mutation: Loop range change
File: src/indexer.rs:620
while batch_size < events.len() {
       ^^^^^^^^^^
       Change to >

Status: Missed (timeout)
```

## Configuration

SorobanPulse's mutation testing is configured in `mutants.toml`:

### Excluded mutations
Some mutations are excluded because they don't test business logic:

```toml
exclude = [
    # Skip comments (already verified by static analysis)
    "regex:^\\s*//.*$",

    # Skip debug assertions (covered by runtime tests)
    "regex:debug_assert",

    # Skip panic messages (tests check panic, not message)
    "regex:panic!.*\",",
]
```

### Module configuration
Specific modules can be targeted:

```toml
[module.src.handlers]
description = "API request handlers"
skip = false  # Include in mutations

[module.src.migrations]
description = "Database migrations"
skip = true   # Don't mutate migrations
```

## Common Mutation Operators

These are typical mutations cargo-mutants introduces:

### Arithmetic operators
```rust
a + b   →   a - b
a * b   →   a / b
a % b   →   a + b
```

### Comparison operators
```rust
a < b   →   a <= b   →   a > b   →   a >= b
a == b  →   a != b
```

### Logical operators
```rust
a && b  →   a || b
!a      →   a
true    →   false
```

### Assignment mutations
```rust
let x = 10;  →   let x = 0;
let x = 10;  →   let x = 1;
```

### Return value mutations
```rust
return x;    →   return !x;
return x;    →   return None;
return x;    →   return 0;
```

## Strategy for High Kill Rate

### 1. Test the happy path
```rust
#[test]
fn test_pagination_offset_valid_inputs() {
    assert_eq!(offset(page: 1, limit: 20), 0);
    assert_eq!(offset(page: 2, limit: 20), 20);
    assert_eq!(offset(page: 3, limit: 20), 40);
}
```

This catches mutations like:
- `(page - 1)` → `page` (changes result for page > 1)
- `(page - 1) * limit` → `(page - 2) * limit` (shifts all offsets)

### 2. Test boundary conditions
```rust
#[test]
fn test_pagination_boundaries() {
    // Test minimum valid page
    assert_eq!(offset(page: 1, limit: 1), 0);
    
    // Test boundary between valid and invalid
    assert_eq!(offset(page: 100, limit: 1), 99);
}
```

This catches mutations like:
- `page >= 1` → `page >= 0` (too permissive)
- `limit <= 100` → `limit <= 101` (off-by-one)

### 3. Test error cases
```rust
#[test]
fn test_pagination_invalid_inputs() {
    assert!(offset(page: 0, limit: 20).is_err());
    assert!(offset(page: 1, limit: 0).is_err());
    assert!(offset(page: 1, limit: 101).is_err());
}
```

This catches mutations like:
- Removing validation checks
- Changing `>` to `>=`

### 4. Test state changes
```rust
#[test]
fn test_subscription_state_transitions() {
    let mut sub = Subscription::new(...);
    
    // Test initial state
    assert_eq!(sub.status(), Status::Inactive);
    
    // Test state change
    sub.activate();
    assert_eq!(sub.status(), Status::Active);
    
    // Test that changes persist
    assert_eq!(sub.status(), Status::Active);
}
```

This catches mutations like:
- Not actually changing state
- Reverting state changes

### 5. Test multiple assertions
```rust
#[test]
fn test_event_filtering_comprehensive() {
    let events = vec![event1, event2, event3];
    let filtered = filter_by_contract(&events, "contract_A");
    
    // Multiple assertions catch different mutations
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.contract_id == "contract_A"));
    assert!(!filtered.contains(&event2));  // Event2 has different contract
}
```

This catches mutations like:
- Wrong filter condition
- Inverting the filter logic
- Off-by-one in count

## Handling Survived Mutations

When a mutation survives, follow this process:

1. **Understand the mutation**
   ```bash
   # View the specific mutation
   cargo mutants -m src/handlers.rs -v | grep "Survived"
   ```

2. **Trace why the test didn't catch it**
   - Is this code path untested?
   - Is the test not specific enough?
   - Is this a false positive?

3. **Add a test that catches it**
   ```rust
   // Example: Add test for specific scenario
   #[test]
   fn test_pagination_different_pages_different_offsets() {
       assert_ne!(offset(page: 1, limit: 20), offset(page: 2, limit: 20));
   }
   ```

4. **Re-run mutation testing**
   ```bash
   cargo mutants -m src/handlers.rs
   ```

5. **Verify it's caught**
   ```bash
   # Check the HTML report
   open target/mutants/index.html
   ```

## CI Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Run mutation tests
  if: github.event_name == 'pull_request'
  run: |
    cargo install cargo-mutants
    cargo mutants --baseline=main
  continue-on-error: true
  
- name: Upload mutation report
  if: always()
  uses: actions/upload-artifact@v3
  with:
    name: mutation-report
    path: target/mutants/
```

## Performance Optimization

Mutation testing can be slow. Optimize with:

### 1. Run selectively on changed files
```bash
# Only mutate files changed in this PR
cargo mutants --check-only -m src/handlers.rs
```

### 2. Use parallel testing
```bash
# Run mutations on 8 CPU cores
cargo mutants -j 8
```

### 3. Set timeouts
```bash
# Kill slow mutations after 30 seconds
timeout 30 cargo test --release
```

### 4. Skip known slow modules
In `mutants.toml`:
```toml
[module.src.slow_integration]
skip = true
```

### 5. Exclude trivial code
In `mutants.toml`:
```toml
exclude = [
    "regex:^\\s*//.*$",      # Comments
    "regex:debug_assert",     # Debug assertions
    "regex:println!",         # Debug output
]
```

## Best Practices

### 1. **Don't aim for 100% mutation kill rate**
- Some code is inherently hard to mutate meaningfully
- Some mutations are equivalent (return the same result)
- Target 90%+ for core business logic, 70%+ for everything else

### 2. **Use mutations to guide tests, not replace them**
- Mutation testing is a diagnostic tool
- Write tests to verify behavior, not to pass mutations
- But if a mutation survives, it indicates an actual test gap

### 3. **Review survived mutations as a team**
- Some survivors might indicate unnecessary code
- Others might reveal important missing test cases
- Document decisions to skip mutations if justified

### 4. **Integrate into CI but don't block merges**
- Mutation testing takes time and resources
- Run as a separate check, allow investigation time
- Use results to guide test improvements

### 5. **Focus on high-value code first**
- Core business logic (calculations, validation, state)
- Error handling and edge cases
- Security-sensitive code

- Less critical: configuration, formatting, logging

## Troubleshooting

### Mutations timeout during test
The mutation caused an infinite loop or slow computation. Common causes:
- Loop condition mutation that never terminates
- Exponential algorithm mutation

Solution: Add timeout to tests in `mutants.toml`:
```toml
[test]
timeout = 10  # Kill after 10 seconds
```

### Too many survived mutations
Indicates test coverage gaps. Solutions:
1. Review the HTML report to identify patterns
2. Add property-based tests (see [property-based-testing.md](property-based-testing.md))
3. Add boundary and error case tests
4. Consider if the code should be refactored to be more testable

### Slow mutation runs
Use the optimization strategies above. Typical runtime:
- Small module (< 500 lines): 1-5 minutes
- Medium module (500-2000 lines): 10-30 minutes
- Large module (> 2000 lines): 30+ minutes

## Resources

- [cargo-mutants documentation](https://docs.rs/cargo-mutants/)
- [Mutation testing concepts](https://en.wikipedia.org/wiki/Mutation_testing)
- [The Pragmatic Programmer: From Journeyman to Master - Testing section](https://pragprog.com/)
- [Property-based testing guide](property-based-testing.md)

## Related Files

- `mutants.toml` - Mutation testing configuration
- `tests/property_tests.rs` - Property-based tests (complementary to mutation testing)
- `.github/workflows/test.yml` - CI pipeline configuration
