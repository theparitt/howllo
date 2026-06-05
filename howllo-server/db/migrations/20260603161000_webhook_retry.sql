ALTER TABLE webhook_deliveries
ADD COLUMN IF NOT EXISTS attempt_number INT NOT NULL DEFAULT 1;

ALTER TABLE webhook_deliveries
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_retry
ON webhook_deliveries (next_retry_at) WHERE success = FALSE AND next_retry_at IS NOT NULL;
