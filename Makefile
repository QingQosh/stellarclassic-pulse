.PHONY: test-unit test-all dev

# Run all unit tests, skipping those that require a real database
test-unit:
	cargo test -- --skip handlers::tests

# Run all tests (requires a running PostgreSQL database and DATABASE_URL in .env)
test-all:
	cargo test

# Run the development server
dev:
	cargo run
