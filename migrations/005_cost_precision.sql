-- Change cost_cents from INTEGER to BIGINT to store micro-cents (1/1_000_000 of a cent).
-- This prevents small requests from rounding to 0.
ALTER TABLE usage_logs ALTER COLUMN cost_cents TYPE BIGINT;

-- Existing rows were stored in cents (mostly 0 due to rounding bug).
-- Multiply by 1_000_000 to convert to micro-cents.
UPDATE usage_logs SET cost_cents = cost_cents * 1000000;
