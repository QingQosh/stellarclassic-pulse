-- tests/e2e/cleanup.sql
--
-- Removes all seeded test data, leaving migrations intact.
-- Run between test suites to ensure a clean state.
--
--   psql "$DATABASE_URL" -f tests/e2e/cleanup.sql

TRUNCATE TABLE events RESTART IDENTITY CASCADE;
