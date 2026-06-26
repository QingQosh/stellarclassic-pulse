# Property-Based Testing Guide

This document describes the property-based testing strategy for SorobanPulse using the `proptest` crate.

## Overview

Property-based testing is a powerful technique that complements traditional unit testing by:

1. **Automatically generating test cases** from specifications
2. **Finding edge cases** that manual tests might miss
3. **Shrinking failures** to minimal reproducible examples
4. **Documenting invariants** that code must maintain

## Why Property-Based Testing?

Traditional example-based tests verify specific scenarios:
```rust
#[test]
fn pagination_works() {
    let page = 1;
    let limit = 20;
    let offset = (page - 1) * limit;
    assert_eq!(offset, 0);
}
```

Property-based tests verify properties that should hold for all valid inputs:
```rust
proptest! {
    #[test]
    fn prop_pagination_offset_formula(page in 1i64..=1000, limit in 1i64..=100) {
        let expected_offset = (page - 1) * limit;
        let actual_offset = (page - 1) * limit;
        assert_eq!(expected_offset, actual_offset);
    }
}
```

The second test runs the same assertion against hundreds of generated page and limit values, catching edge cases automatically.

## Running Property Tests

```bash
# Run all property tests
cargo test --test property_tests

# Run with verbose output to see generated examples
PROPTEST_VERBOSE=1 cargo test --test property_tests

# Run a specific property test
cargo test --test property_tests prop_pagination_offset_formula

# Increase the number of test cases (default: 256)
PROPTEST_CASES=1000 cargo test --test property_tests

# Use specific random seed for reproducibility
PROPTEST_RNG_SEED=12345 cargo test --test property_tests
```

## Test Structure

Property tests are organized in `tests/property_tests.rs` by domain:

### 1. Pagination Tests

**Properties verified:**
- Offset formula: `offset = (page - 1) * limit`
- Limit clamping respects min/max bounds (1-100)
- Page normalization ensures positive pages
- Offset is never negative for valid inputs

**Tests:**
- `prop_pagination_offset_formula` - Formula correctness
- `prop_limit_clamping_max` - Upper bound enforcement
- `prop_limit_clamping_min` - Lower bound enforcement
- `prop_limit_valid_range` - Range constraints
- `prop_page_normalization` - Page normalization
- `prop_offset_non_negative` - Non-negative offsets

**Examples of caught bugs:**
- Off-by-one errors in offset calculation
- Missing min/max clamping on limits
- Integer overflow in large page numbers

### 2. Ledger Range Filter Tests

**Properties verified:**
- Ledger ranges are inclusive on both boundaries
- Events outside the range are correctly excluded
- Ledger numbers are never negative
- Boundaries are correctly enforced

**Tests:**
- `prop_ledger_range_inclusive` - Inclusive boundaries
- `prop_ledger_from_boundary` - Lower bound exclusion
- `prop_ledger_to_boundary` - Upper bound exclusion
- `prop_ledger_always_positive` - Non-negative ledgers

**Examples of caught bugs:**
- Off-by-one errors in range checks
- Exclusive vs inclusive boundary confusion
- Swapped from/to boundaries

### 3. Timestamp Filter Tests

**Properties verified:**
- ISO 8601 timestamps round-trip correctly
- Timestamp ranges are inclusive on both boundaries
- Events outside the range are correctly excluded
- Parsing is consistent across formats

**Tests:**
- `prop_timestamp_roundtrip` - Parsing consistency
- `prop_timestamp_range_inclusive` - Inclusive boundaries
- `prop_timestamp_from_boundary` - Lower bound exclusion
- `prop_timestamp_to_boundary` - Upper bound exclusion

**Examples of caught bugs:**
- Timezone conversion errors
- Microsecond precision loss
- Boundary off-by-one in millisecond comparisons

### 4. Contract ID Validation Tests

**Properties verified:**
- Valid contract IDs match Stellar format (C + 55 base32 chars)
- Contract ID filtering is case-sensitive
- Invalid formats are consistently rejected

**Tests:**
- `prop_contract_id_format` - Format validation
- `prop_contract_id_case_sensitive` - Case sensitivity

**Examples of caught bugs:**
- Improper case normalization
- Missing format validation
- Boundary errors in length checks

### 5. Transaction Hash Validation Tests

**Properties verified:**
- Transaction hashes are 64 lowercase hex characters
- Hash parsing is consistent
- Invalid formats are rejected

**Tests:**
- `prop_tx_hash_format` - Format validation

**Examples of caught bugs:**
- Uppercase hex handling inconsistency
- Incorrect hash length validation

### 6. Combined Filter Tests

**Properties verified:**
- Multiple filters can be applied independently
- Filter composition doesn't have unintended interactions
- Results correctly satisfy all filter constraints

**Tests:**
- `prop_combined_ledger_timestamp_filters` - Filter independence

**Examples of caught bugs:**
- Filters interfering with each other
- AND logic incorrectly implemented as OR
- Missing null/None handling in combined filters

## Custom Strategies

Strategies define how proptest generates input values. Custom strategies are defined in `tests/property_tests.rs`:

```rust
/// Strategy for generating valid page numbers
fn page_strategy() -> impl Strategy<Value = i64> {
    1i64..=1000
}

/// Strategy for generating valid contract IDs (Stellar contract format)
fn contract_id_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("C[A-Z2-7]{55}")
        .expect("valid contract ID regex")
}
```

### Common Strategy Combinators

```rust
// Range strategy
1i64..=100i64

// Option strategy (None or Some(value))
prop_oneof![
    Just(None),
    value_strategy.prop_map(Some),
]

// Vector strategy
prop::collection::vec(item_strategy, 0..10)

// Tuple strategy
(strategy1, strategy2, strategy3)

// Weighted strategy
prop_oneof![
    2 => value_strategy1,  // 2x probability
    1 => value_strategy2,  // 1x probability
]

// Regular expression strategy
prop::string::string_regex("[a-z]+").unwrap()

// Filtered strategy
base_strategy.prop_filter("description", |val| val > 10)
```

## Shrinking

When proptest finds a failing test case, it automatically shrinks the input to find the minimal reproducer:

```
thread 'prop_pagination_offset_formula' panicked at 'assertion failed: offset >= 0'
shrinking 1000 iterations
found error after 42 iterations, shrunk to:
  page: -5
  limit: 0
```

This helps you understand the root cause. The library tries increasingly simpler values until it finds the boundary that causes the failure.

## Integration into CI

To integrate property tests into CI:

```bash
# In .github/workflows/test.yml
- name: Run property tests
  run: cargo test --test property_tests

# With shrinking disabled (for faster CI)
- name: Run property tests (fast mode)
  run: PROPTEST_CASES=100 cargo test --test property_tests
```

## Extending Property Tests

To add new property tests:

1. **Define a strategy** for your input domain:
   ```rust
   fn my_value_strategy() -> impl Strategy<Value = MyType> {
       // Generate MyType values
   }
   ```

2. **Write the property**:
   ```rust
   proptest! {
       #[test]
       fn prop_my_property(value in my_value_strategy()) {
           // Verify the property holds
           prop_assert!(invariant_holds(&value));
       }
   }
   ```

3. **Add documentation**:
   ```rust
   /// Property: Description of what should always be true
   ///
   /// Explain why this property matters and what would indicate a bug.
   ```

## Best Practices

### 1. **Name properties clearly**
```rust
// Good
#[test]
fn prop_limit_clamping_respects_max_bound() { }

// Avoid
#[test]
fn prop_test_1() { }
```

### 2. **Document the property**
```rust
/// Property: Ledger range filters are inclusive on both ends
///
/// An event at ledger N should be included if from_ledger <= N <= to_ledger
#[test]
fn prop_ledger_range_inclusive(...) { }
```

### 3. **Keep strategies realistic**
```rust
// Good: validates realistic input ranges
fn contract_id_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("C[A-Z2-7]{55}").unwrap()
}

// Avoid: testing unrealistic values
fn contract_id_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[^a-zA-Z0-9]*").unwrap()
}
```

### 4. **Use meaningful assertions**
```rust
// Good: clear error message
prop_assert!(offset >= 0, "offset {} is negative", offset);

// Avoid: cryptic assertion
prop_assert!(offset >= 0);
```

### 5. **Test invariants, not implementations**
```rust
// Good: tests what should be true
#[test]
fn prop_any_clamped_value_in_range(val in -1000i64..=2000) {
    let clamped = val.clamp(1, 100);
    prop_assert!(clamped >= 1 && clamped <= 100);
}

// Avoid: testing specific implementation
#[test]
fn prop_clamp_subtracts_one(val in 101..=2000) {
    assert_eq!(val.clamp(1, 100), val - 1); // brittle
}
```

## Troubleshooting

### Test fails intermittently

This shouldn't happen with proptest — property tests are deterministic given the same seed. If you see intermittent failures:

1. Check for use of random data not controlled by the strategy
2. Check for timing-dependent assertions
3. Use `PROPTEST_RNG_SEED=<seed>` to reproduce

### Test is too slow

Property tests generate many cases by default (256). To speed up:

```bash
PROPTEST_CASES=50 cargo test --test property_tests
```

Or reduce strategy complexity in your `Arbitrary` implementations.

### Strategy generates invalid values

If your strategy generates values outside the intended domain:

1. Use `.prop_filter()` to reject invalid values:
   ```rust
   ledger_strategy().prop_filter("positive", |&val| val >= 0)
   ```

2. Or use a more constrained strategy:
   ```rust
   0i64..=u32::MAX as i64  // Only valid ledger numbers
   ```

## Resources

- [proptest documentation](https://docs.rs/proptest/)
- [Proptest book](https://docs.rs/proptest/latest/proptest/)
- [Property-based testing concepts](https://hypothesis.works/articles/what-is-property-based-testing/)
- [Shrinking in property-based testing](https://hypothesis.works/articles/what-is-shrinking/)

## Related Documentation

- [Integration tests](https://docs.rs/soroban-pulse) - Full system integration tests
- [Benchmarks](../benches/) - Performance benchmarks
- [CI/CD pipeline](.github/workflows/) - Automated testing in CI
