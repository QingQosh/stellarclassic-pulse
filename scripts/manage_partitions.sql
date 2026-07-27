-- Script for automatic partition generation and management
-- This script creates functions to automatically manage event table partitions

-- Function to create a new partition for a given month
CREATE OR REPLACE FUNCTION create_event_partition(partition_year INT, partition_month INT)
RETURNS void AS $$
DECLARE
    partition_name TEXT;
    start_date DATE;
    end_date DATE;
BEGIN
    partition_name := 'events_' || partition_year || '_' ||
                      LPAD(partition_month::TEXT, 2, '0');
    start_date := make_date(partition_year, partition_month, 1);
    end_date := start_date + INTERVAL '1 month';

    -- Create the partition if it doesn't exist
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF events
         FOR VALUES FROM (%L) TO (%L)',
        partition_name,
        start_date,
        end_date
    );

    -- Create indexes on the new partition
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_%s_contract_id ON %I(contract_id)',
                   partition_name, partition_name);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_%s_tx_hash ON %I(tx_hash)',
                   partition_name, partition_name);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_%s_ledger ON %I(ledger)',
                   partition_name, partition_name);

    RAISE NOTICE 'Partition % created successfully', partition_name;
END;
$$ LANGUAGE plpgsql;

-- Function to create partitions for the next N months
CREATE OR REPLACE FUNCTION create_future_partitions(months_ahead INT DEFAULT 3)
RETURNS void AS $$
DECLARE
    current_date DATE;
    target_date DATE;
    year_val INT;
    month_val INT;
BEGIN
    current_date := DATE_TRUNC('month', NOW())::DATE;
    target_date := current_date + (months_ahead || ' months')::INTERVAL;

    WHILE current_date <= target_date LOOP
        year_val := EXTRACT(YEAR FROM current_date)::INT;
        month_val := EXTRACT(MONTH FROM current_date)::INT;

        PERFORM create_event_partition(year_val, month_val);

        current_date := current_date + INTERVAL '1 month';
    END LOOP;

    RAISE NOTICE 'Created partitions through %', target_date;
END;
$$ LANGUAGE plpgsql;

-- Function to clean up old partitions (older than retention period)
CREATE OR REPLACE FUNCTION drop_old_partitions(retention_months INT DEFAULT 12)
RETURNS void AS $$
DECLARE
    partition_record RECORD;
    cutoff_date DATE;
BEGIN
    cutoff_date := NOW() - (retention_months || ' months')::INTERVAL;

    FOR partition_record IN
        SELECT schemaname, tablename
        FROM pg_tables
        WHERE schemaname = 'public'
          AND tablename LIKE 'events_%'
          AND tablename ~ '^\d{4}_\d{2}$'
    LOOP
        IF TO_DATE(partition_record.tablename, 'YYYY_MM') < cutoff_date THEN
            EXECUTE format('DROP TABLE IF EXISTS %I', partition_record.tablename);
            RAISE NOTICE 'Dropped partition %', partition_record.tablename;
        END IF;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Function to refresh partition statistics
CREATE OR REPLACE FUNCTION refresh_partition_stats()
RETURNS void AS $$
DECLARE
    partition_record RECORD;
BEGIN
    FOR partition_record IN
        SELECT schemaname, tablename
        FROM pg_tables
        WHERE schemaname = 'public'
          AND (tablename = 'events' OR tablename LIKE 'events_%')
    LOOP
        EXECUTE format('ANALYZE %I', partition_record.tablename);
    END LOOP;

    RAISE NOTICE 'Partition statistics refreshed';
END;
$$ LANGUAGE plpgsql;

-- Function to enable partition pruning on queries
CREATE OR REPLACE FUNCTION enable_partition_pruning()
RETURNS void AS $$
BEGIN
    SET constraint_exclusion = partition;
    SET enable_partition_pruning = on;
    RAISE NOTICE 'Partition pruning enabled';
END;
$$ LANGUAGE plpgsql;

-- Create scheduled job for automatic partition creation (requires pg_cron extension)
-- This comment shows how to set it up if pg_cron is available:
-- SELECT cron.schedule('create-future-partitions', '0 0 1 * *', 'SELECT create_future_partitions(3)');
-- SELECT cron.schedule('drop-old-partitions', '0 0 15 * *', 'SELECT drop_old_partitions(12)');
-- SELECT cron.schedule('refresh-partition-stats', '0 2 * * *', 'SELECT refresh_partition_stats()');
