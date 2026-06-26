# Property-Based Testing Guide

## Overview

This project uses **property-based testing** with the [`proptest`](https://github.com/AltSysRq/proptest) crate to improve test coverage by automatically discovering edge cases and verifying invariants across randomly generated inputs.

## Why Property-Based Testing?

Traditional unit tests verify specific examples. Property-based tests verify **invariants** that should hold for entire classes of inputs:

### Traditional (Example-Based)
```rust
#[test]
fn pagination_offset_for_page_3_limit_10() {
    let params = PaginationParams { page: Some(3), limit: Some(10) };
    assert_eq!(params.offset(), 20);
}
```

### Property-Based
```rust
#[test]
fn prop_pagination_offset_is_nonnegative(
    page in page_strategy(),
    limit in limit_strategy()
) {
    let params = PaginationParams { page, limit };
    assert!(params.offset() >= 0);  // Always true for ANY inputs
}
```

## Running Property Tests

### Run all tests
```bash
cargo test --test property_tests
```

### Run with custom seed (reproducible failures)
```bash
PROPTEST_RNG_SEED=seed cargo test --test property_tests
```

### Verbose output (see generated test cases)
```bash
PROPTEST_VERBOSE=1 cargo test --test property_tests
```

### Increase test case count (more thorough)
```bash
PROPTEST_CASES=10000 cargo test --test property_tests
```

## Test Structure

Property tests are organized in `tests/property_tests.rs` by domain:

### 1. Pagination Tests (`prop_pagination_*`)
Verify that pagination parameters (page, limit) are validated correctly:
- Limits are clamped to [1, 100]
- Pages are clamped to minimum 1
- Offset calculation is correct and never negative
- Defaults are applied consistently
- Zero and negative values are handled gracefully

**Strategies Used:**
- `page_strategy()`: Generates valid and invalid page numbers
- `limit_strategy()`: Generates valid and invalid limit values

**Example Test:**
```rust
proptest! {
    #[test]
    fn prop_pagination_limit_within_bounds(limit in limit_strategy()) {
        let params = PaginationParams { page: None, limit };
        let resolved = params.limit();
        assert!(resolved >= 1 && resolved <= 100);
    }
}
```

### 2. Timestamp Tests (`prop_timestamp_*`)
Verify ISO 8601 timestamp handling:
- Valid RFC3339 strings parse successfully
- Invalid strings are rejected
- Roundtrip serialization preserves values
- Temporal ordering is preserved
- Duration arithmetic is consistent

**Strategies Used:**
- `timestamp_strategy()`: Generates random valid timestamps
- `iso8601_timestamp_strategy()`: Generates valid RFC3339 strings
- `invalid_timestamp_strategy()`: Generates malformed strings

**Example Test:**
```rust
proptest! {
    #[test]
    fn prop_timestamp_roundtrip_preserves_value(
        ts_str in iso8601_timestamp_strategy()
    ) {
        let parsed = validate_timestamp(&ts_str).unwrap();
        let serialized = parsed.to_rfc3339();
        let reparsed = validate_timestamp(&serialized).unwrap();
        assert_eq!(parsed, reparsed);
    }
}
```

### 3. Filter Validation Tests (`prop_*_validation`)
Verify that input filters are validated correctly:
- Contract IDs must be 56-character hex strings
- Ledger ranges are valid when from <= to
- Timestamp ranges are valid when from <= to

**Strategies Used:**
- `contract_id_strategy()`: Generates valid and invalid contract IDs
- Custom integer ranges for ledger numbers
- `timestamp_strategy()` pairs for time ranges

**Example Test:**
```rust
proptest! {
    #[test]
    fn prop_ledger_range_validation(
        from in 0i64..=1_000_000,
        to in 0i64..=1_000_000
    ) {
        if from > to {
            assert!(!is_valid_ledger_range(from, to));
        } else {
            assert!(is_valid_ledger_range(from, to));
        }
    }
}
```

## Strategies: Controlling Input Generation

A **strategy** defines the space of inputs that proptest generates. The project uses several strategies:

### Basic Strategies
```rust
// Ranges
(1i64..=100).prop_map(Some)  // 1 to 100, wrapped in Option

// Alternatives (OR logic)
prop_oneof![
    Just(None),           // Always generate None
    Just(Some(0)),        // Always generate Some(0)
    (1..100).prop_map(Some),  // Random 1-100
]

// Mapped/transformed values
timestamp_strategy()
    .prop_map(|ts| ts.to_rfc3339())  // Convert to string
```

### Custom Strategies
Define strategies for domain-specific values:

```rust
fn contract_id_strategy() -> impl Strategy<Value = String> {
    "[0-9a-f]{56}".prop_map(|s| s)  // Regex-based generation
}
```

### Combining Strategies
```rust
proptest! {
    #[test]
    fn test_with_multiple_inputs(
        page in page_strategy(),
        limit in limit_strategy(),
        contract_id in contract_id_strategy()
    ) {
        // This test receives 3 random inputs per case
    }
}
```

## Shrinking

When a test fails, **proptest automatically shrinks** the failing input to a minimal reproducible example:

```
thread 'prop_pagination_limit_within_bounds' panicked at 'limit should be between 1 and 100, got -999999 for Some(-999999)'

Shrunk to: limit = Some(-1)
Shrunk to: limit = Some(0)
```

This makes debugging much easier — you get the smallest input that triggers the bug.

## Adding New Property Tests

Follow this pattern:

```rust
proptest! {
    #[test]
    fn prop_your_test_name(
        input1 in strategy1(),
        input2 in strategy2(),
    ) {
        // Setup
        let result = function_under_test(input1, input2);

        // Assert an invariant (should be true for ALL inputs)
        prop_assert!(result.is_valid());
        
        // Or assert equality
        prop_assert_eq!(result.len(), input1.len());
    }
}
```

**Guidelines:**
1. Name tests with `prop_` prefix for clarity
2. Use `prop_assert!` or `prop_assert_eq!` instead of `assert!`
3. Focus on **invariants**, not specific values
4. Include comments explaining what the invariant is
5. Provide meaningful error messages in assertions

## Integration with CI

Property tests are automatically run as part of the test suite:

```bash
make test   # Runs all tests including property tests
```

To increase CI test intensity:

```bash
PROPTEST_CASES=10000 cargo test --test property_tests
```

Add this to `.github/workflows/test.yml`:
```yaml
- name: Run property tests
  env:
    PROPTEST_CASES: 10000
  run: cargo test --test property_tests
```

## Common Patterns

### Testing bounds validation
```rust
proptest! {
    #[test]
    fn prop_value_is_clamped(value in i64::MIN..=i64::MAX) {
        let clamped = clamp(value, 0, 100);
        assert!(clamped >= 0 && clamped <= 100);
    }
}
```

### Testing invariants
```rust
proptest! {
    #[test]
    fn prop_set_size_is_consistent(items in vec(any::<i32>(), 0..100)) {
        let mut set = MySet::new();
        for item in items {
            set.insert(item);
        }
        assert_eq!(set.len(), items.iter().unique().count());
    }
}
```

### Testing ordering
```rust
proptest! {
    #[test]
    fn prop_sort_produces_ordered_output(
        mut items in vec(0i32..100, 0..100)
    ) {
        items.sort();
        for i in 0..items.len()-1 {
            assert!(items[i] <= items[i+1]);
        }
    }
}
```

## Performance Considerations

Property tests generate 256 test cases by default (`PROPTEST_CASES=256`). On large datasets:

- **Limit test size**: Use `vec(strategy, 0..100)` instead of unbounded
- **Cache expensive setup**: Use `proptest::prelude::proptest!` block scope
- **Consider CI intensity**: Use lower PROPTEST_CASES for local tests, higher for CI

## Troubleshooting

### Test takes too long
Reduce the strategy scope:
```rust
// Bad: unbounded
(i64::MIN..=i64::MAX)

// Good: reasonable bounds
(0i64..=1_000_000)
```

### Test fails with "state is empty after shrinking"
Your assertions may be too strict. Ensure they check actual invariants, not specific values.

### "failed to find a failing input"
The invariant might actually be false. Review the logic carefully.

## Resources

- [proptest Book](https://docs.rs/proptest/latest/proptest/)
- [Quickcheck comparison](https://github.com/AltSysRq/proptest/wiki/FAQ#how-do-i-compare-proptest-and-quickcheck)
- [Strategy Guide](https://docs.rs/proptest/latest/proptest/arbitrary/index.html)
