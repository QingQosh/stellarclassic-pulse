# Webhook endpoint rate limits

Webhook deliveries track per-endpoint state in `rate_limit_endpoints`.

Operators can tune `per_minute_limit` per `endpoint_url`. When an endpoint
exceeds its limit or is in backoff, new webhook payloads are inserted into
`webhook_retry_queue` with a future `next_retry_at`.

Delivery failures update endpoint health:

- `healthy`: last delivery succeeded
- `degraded`: recent failures below the unhealthy threshold
- `unhealthy`: three or more consecutive failures

Backoff grows exponentially and is capped at 15 minutes.
