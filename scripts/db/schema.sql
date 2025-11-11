-- Example PostgreSQL schema for storing OpenZeppelin Monitor notifications
-- This table stores notifications from the database trigger type
-- Optimized to store only decoded/processed data, not raw transaction bytes

CREATE TABLE IF NOT EXISTS monitor_notifications (
    id SERIAL PRIMARY KEY,
    
    -- Transaction identifiers
    transaction_hash TEXT NOT NULL,
    block_number BIGINT,
    network TEXT NOT NULL,
    
    -- Monitor identifiers
    monitor_name TEXT NOT NULL,
    
    -- Decoded/processed data only
    matched_conditions JSONB NOT NULL,         -- What conditions triggered the match
    decoded_args JSONB,                        -- Decoded function/event parameters
    variables JSONB NOT NULL DEFAULT '{}',     -- Template variables for notifications
    additional_fields JSONB NOT NULL DEFAULT '{}',  -- Custom fields from trigger config
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Add indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_monitor_notifications_created_at 
    ON monitor_notifications(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_monitor_notifications_tx_hash 
    ON monitor_notifications(transaction_hash);

CREATE INDEX IF NOT EXISTS idx_monitor_notifications_network 
    ON monitor_notifications(network);

CREATE INDEX IF NOT EXISTS idx_monitor_notifications_monitor_name 
    ON monitor_notifications(monitor_name);

CREATE INDEX IF NOT EXISTS idx_monitor_notifications_decoded_args 
    ON monitor_notifications USING GIN (decoded_args);

CREATE INDEX IF NOT EXISTS idx_monitor_notifications_additional_fields 
    ON monitor_notifications USING GIN (additional_fields);

-- Optional: Add comments to describe the table and columns
COMMENT ON TABLE monitor_notifications IS 'Stores blockchain monitor notifications from OpenZeppelin Monitor with only decoded/processed data';
COMMENT ON COLUMN monitor_notifications.transaction_hash IS 'Transaction hash that triggered the notification';
COMMENT ON COLUMN monitor_notifications.block_number IS 'Block number containing the transaction';
COMMENT ON COLUMN monitor_notifications.network IS 'Network slug (e.g., ethereum_mainnet, polygon_mainnet)';
COMMENT ON COLUMN monitor_notifications.monitor_name IS 'Name of the monitor that triggered';
COMMENT ON COLUMN monitor_notifications.matched_conditions IS 'Conditions that were matched (functions, events, transactions)';
COMMENT ON COLUMN monitor_notifications.decoded_args IS 'Decoded function and event arguments from matched conditions';
COMMENT ON COLUMN monitor_notifications.variables IS 'Template variables available for notifications';
COMMENT ON COLUMN monitor_notifications.additional_fields IS 'Custom metadata fields defined in trigger configuration';
