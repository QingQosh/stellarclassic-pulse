# Operator Runbook

Procedures for common operational tasks, incident response, and system maintenance for Soroban Pulse.

## Table of Contents

- [Emergency Response Procedures](#emergency-response-procedures)
- [Data Corruption Recovery](#data-corruption-recovery)
- [Backlog Clearing Procedures](#backlog-clearing-procedures)
- [Replica Failover Procedures](#replica-failover-procedures)
- [Health Check Interpretation](#health-check-interpretation)
- [Debugging Checklists](#debugging-checklists)
- [Escalation Paths](#escalation-paths)

---

## Emergency Response Procedures

### Severity levels

| Level | Definition | Response time |
|-------|------------|---------------|
| SEV-1 | Complete outage — no events being indexed, API down | Immediate |
| SEV-2 | Partial degradation — indexer lagging > 500 ledgers, error rate > 5% | < 15 minutes |
| SEV-3 | Degraded performance — high latency, indexer lagging 100–500 ledgers | < 1 hour |
| SEV-4 | Minor issue — single failing webhook subscription, minor config drift | Best effort |

---

### SEV-1: Service is completely down

**Symptoms**: `/healthz/ready` returns `503`; no events being indexed; API unreachable.

**Step 1 — Triage**
```bash
# Is the process running?
kubectl get pods -l app=soroban-pulse

# What is the health endpoint saying?
curl http://<service>/healthz/ready
curl http://<service>/healthz/live
```

**Step 2 — Check the database**
```bash
psql $DATABASE_URL -c "SELECT 1;"
# If this fails, the DB is the problem — see "Database is unreachable" below
```

**Step 3 — Check logs for panic or startup failure**
```bash
kubectl logs -l app=soroban-pulse --tail=200 | grep -i "panic\|error\|fatal"
```

**Step 4 — Restart the service**
```bash
kubectl rollout restart deployment/soroban-pulse
kubectl rollout status deployment/soroban-pulse
```

**Step 5 — Verify recovery**
```bash
curl http://<service>/healthz/ready
# Expected: {"status":"ok","db":"ok","indexer":"ok"}
```

---

### Database is unreachable

```bash
# 1. Check DB pod / RDS status
kubectl get pods -l app=postgres
# or check your cloud console for RDS / Cloud SQL status

# 2. Verify credentials have not rotated
psql $DATABASE_URL -c "SELECT current_user;"

# 3. Check network policy / security groups allow traffic on 5432

# 4. If using Docker Compose, restart the DB service
docker compose restart db
docker compose logs db --tail=50
```

---

### RPC endpoint is unreachable

```bash
# 1. Test RPC connectivity directly
curl -s $STELLAR_RPC_URL/health | jq .

# 2. Switch to backup RPC endpoint
kubectl set env deployment/soroban-pulse \
  STELLAR_RPC_URL=https://soroban-testnet.stellar.org
kubectl rollout restart deployment/soroban-pulse
```

See [docs/runbooks/rpc-errors.md](rpc-errors.md) for the full RPC runbook.

---

### Out-of-memory / OOM kill

**Symptoms**: Pod is in `OOMKilled` state; `soroban_pulse_process_memory_bytes` exceeded the container limit.

```bash
# 1. Confirm OOM kill
kubectl describe pod <pod-name> | grep -i "oom\|killed\|reason"

# 2. Check current memory usage
kubectl top pod -l app=soroban-pulse

# 3. Temporary fix: increase memory limit
kubectl set resources deployment/soroban-pulse \
  --limits=memory=1Gi
```

Longer-term: investigate whether a memory leak exists (check SSE connection count, query result set sizes).

---

## Data Corruption Recovery

### Identify corruption

```sql
-- Check for events with NULL required fields
SELECT COUNT(*) FROM events
WHERE contract_id IS NULL
   OR ledger IS NULL
   OR tx_hash IS NULL;

-- Check for out-of-order ledgers (should be monotonically increasing per contract)
SELECT contract_id, COUNT(*) AS gaps
FROM (
    SELECT contract_id, ledger,
           LAG(ledger) OVER (PARTITION BY contract_id ORDER BY ledger) AS prev_ledger
    FROM events
) t
WHERE ledger < prev_ledger
GROUP BY contract_id;
```

### Re-index from a known good ledger

If events are corrupted or missing for a ledger range, use the admin replay endpoint to re-index:

```bash
# Replay events from ledger 1234000 to 1235000
curl -X POST http://localhost:3000/v1/admin/indexer/replay \
  -H "X-Api-Key: $ADMIN_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"from_ledger": 1234000, "to_ledger": 1235000}'
```

The replay inserts with `ON CONFLICT DO NOTHING`, so duplicate events are safely skipped.

### Restore from backup

If data corruption is widespread, restore from a PostgreSQL backup:

```bash
# 1. Stop the service to prevent writes during restore
kubectl scale deployment/soroban-pulse --replicas=0

# 2. Restore the database
./scripts/restore.sh <backup-file.dump>

# 3. Verify the restored data
psql $DATABASE_URL -c "SELECT MAX(ledger), COUNT(*) FROM events;"

# 4. Restart the service
kubectl scale deployment/soroban-pulse --replicas=1
kubectl rollout status deployment/soroban-pulse
```

See [docs/backup-verification.md](../backup-verification.md) for backup and restore procedures.

### Anonymize corrupted PII data

If a data incident exposes personally-identifiable information in event data:

```bash
# Trigger anonymization via the admin endpoint
curl -X POST http://localhost:3000/v1/admin/anonymize \
  -H "X-Api-Key: $ADMIN_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"contract_id": "CABC...", "field": "email"}'
```

See [docs/encryption.md](../encryption.md) for the full data protection reference.

---

## Backlog Clearing Procedures

### Indexer lag (backlog > 500 ledgers)

When the indexer has fallen significantly behind the network tip, it will catch up automatically but may take time. To speed recovery:

**1. Verify the indexer is the active leader:**
```bash
curl http://localhost:3000/metrics | grep soroban_pulse_indexer_is_leader
# Should be 1
```

**2. Check the lag trend (is it improving or worsening?):**
```bash
curl http://localhost:3000/metrics | grep soroban_pulse_indexer_lag_ledgers
```

Watch this over 2–3 minutes. If the lag is decreasing, the indexer is catching up — leave it running.

**3. If lag is stable or increasing, check for a bottleneck:**
```bash
# Is the RPC endpoint responsive?
curl -s $STELLAR_RPC_URL/health | jq .

# Are database inserts slow?
psql $DATABASE_URL -c "
  SELECT mean_exec_time::int AS mean_ms, query
  FROM pg_stat_statements
  WHERE query ILIKE '%INSERT INTO events%'
  ORDER BY mean_exec_time DESC LIMIT 5;"
```

**4. If the database is the bottleneck, increase the pool:**
```bash
kubectl set env deployment/soroban-pulse DB_MAX_CONNECTIONS=20
kubectl rollout restart deployment/soroban-pulse
```

See [docs/runbooks/indexer-lag.md](indexer-lag.md) for the full runbook.

---

### Webhook delivery backlog

Failed webhooks are retried automatically with exponential backoff. If the backlog is large:

**1. Check the failure count:**
```bash
curl http://localhost:3000/metrics | grep soroban_pulse_webhook_failures_total
```

**2. Identify failing subscriptions:**
```sql
SELECT id, url, status, last_error, failed_at
FROM webhooks
WHERE status = 'failed'
ORDER BY failed_at DESC
LIMIT 20;
```

**3. Test the subscriber endpoint:**
```bash
curl -X POST https://subscriber.example.com/hook \
  -H "Content-Type: application/json" \
  -d '{"test":true}'
```

**4. If the subscriber is permanently down, disable the subscription:**
```bash
curl -X DELETE http://localhost:3000/v1/admin/webhooks/<id> \
  -H "X-Api-Key: $ADMIN_API_KEY"
```

See [docs/runbooks/webhook-failures.md](webhook-failures.md) for the full runbook.

---

### Notification queue backlog

```bash
# Check email failure count
curl http://localhost:3000/metrics | grep soroban_pulse_email_failures_total

# Pause notifications while investigating
curl -X POST http://localhost:3000/v1/admin/indexer/pause \
  -H "X-Api-Key: $ADMIN_API_KEY"

# Resume after fixing the underlying issue
curl -X POST http://localhost:3000/v1/admin/indexer/resume \
  -H "X-Api-Key: $ADMIN_API_KEY"
```

---

## Replica Failover Procedures

### How advisory lock failover works

Soroban Pulse uses a PostgreSQL session-level advisory lock (`pg_try_advisory_lock`) for leader election:

1. On startup, each replica tries to acquire the lock
2. The first replica to succeed becomes the **active indexer** (leader)
3. Other replicas enter a **standby retry loop**, checking every `INDEXER_LOCK_RETRY_SECS` seconds
4. When the leader's DB connection is dropped (crash, network partition, restart), PostgreSQL releases the lock automatically
5. A standby acquires the lock within one retry interval and starts indexing

No manual intervention is required for normal failover.

### Manual failover (force leadership change)

To move indexing to a different replica (e.g., before planned maintenance):

```bash
# 1. Identify which pod holds the lock
kubectl exec -it <pod-name> -- \
  curl -s http://localhost:3000/metrics | grep soroban_pulse_indexer_is_leader

# 2. Gracefully restart the current leader
kubectl rollout restart deployment/soroban-pulse --timeout=60s

# 3. Verify a standby has taken over (within INDEXER_LOCK_RETRY_SECS)
kubectl exec -it <standby-pod> -- \
  curl -s http://localhost:3000/metrics | grep soroban_pulse_indexer_is_leader
# Should become 1 on the standby
```

### Verify no split-brain

After failover, confirm exactly one replica holds the lock:

```sql
SELECT pid, granted FROM pg_locks WHERE locktype = 'advisory';
```

Only one row should appear. Multiple rows with `granted = true` indicates a split-brain — restart all replicas immediately.

```bash
kubectl rollout restart deployment/soroban-pulse
```

### Replica promotion in Kubernetes

If using HPA (Horizontal Pod Autoscaler):

```bash
# Scale down to 1 replica to force a single leader
kubectl scale deployment/soroban-pulse --replicas=1

# Scale back up after verifying stability
kubectl scale deployment/soroban-pulse --replicas=3
```

---

## Health Check Interpretation

### Endpoint reference

| Endpoint | Purpose | Expected response |
|----------|---------|-------------------|
| `GET /healthz/live` | Process is alive | `200 {"status":"alive"}` |
| `GET /healthz/ready` | DB + indexer healthy | `200 {"status":"ok","db":"ok","indexer":"ok"}` |
| `GET /health` | Alias for `/healthz/ready` | Same as above |

### Response meanings

**`/healthz/live` returns 200**
The process is running. This check requires no external dependencies. Kubernetes uses this as the liveness probe — a failure triggers a pod restart.

**`/healthz/ready` returns 200**
```json
{"status": "ok", "db": "ok", "indexer": "ok"}
```
Both the database ping succeeded and the indexer has not stalled. Kubernetes uses this as the readiness probe — only pods that pass this check receive traffic.

**`/healthz/ready` returns 503**
```json
{"status": "degraded", "db": "ok", "indexer": "stalled"}
```
or
```json
{"status": "degraded", "db": "error", "indexer": "unknown"}
```

| Field | Value | Meaning |
|-------|-------|---------|
| `db` | `"ok"` | Database ping succeeded |
| `db` | `"error"` | Database unreachable or slow (exceeds `HEALTH_CHECK_TIMEOUT_MS`) |
| `indexer` | `"ok"` | Indexer processed a ledger recently |
| `indexer` | `"stalled"` | No ledger progress for > 2× the expected poll interval |
| `indexer` | `"standby"` | This replica does not hold the advisory lock (normal for standbys) |

**`HEALTH_CHECK_TIMEOUT_MS`** (default: `2000`) — the health check DB ping will fail if it does not complete within this window. Raise it if your database has high latency during startup.

---

## Debugging Checklists

### Checklist: Service not starting

- [ ] Is `DATABASE_URL` set and points to a reachable host?
- [ ] Does the database user have `CREATE TABLE`, `INSERT`, `SELECT` privileges?
- [ ] Are migrations blocked by an existing failed migration? (`SELECT * FROM _sqlx_migrations WHERE success = false`)
- [ ] Is `RUST_LOG` set? (Without it, startup errors may be silent)
- [ ] Is `ADMIN_API_KEY` required by your deployment and set?
- [ ] Is the port (`PORT`) already in use?

### Checklist: Indexer not making progress

- [ ] Does `soroban_pulse_indexer_is_leader` == 1 on this replica?
- [ ] Is the RPC endpoint reachable? (`curl $STELLAR_RPC_URL/health`)
- [ ] Is `START_LEDGER` set to a valid ledger within the RPC history window?
- [ ] Are there database errors in the logs? (`RUST_LOG=debug`)
- [ ] Is the connection pool exhausted? (`pool_size` == `pool_max`?)
- [ ] Is there a migration that has not completed successfully?

### Checklist: API returning errors or stale data

- [ ] Is `/healthz/ready` returning `200`?
- [ ] Is the indexer lag below the warning threshold?
- [ ] Are there slow queries? (`SLOW_QUERY_THRESHOLD_MS` logs)
- [ ] Is the rate limiter rejecting requests? (`soroban_pulse_rate_limit_rejected_total` rate)
- [ ] Is the correct `API_KEY` being sent (if auth is enabled)?
- [ ] Are filters using indexed columns? (`contract_id`, `ledger`, `tx_hash`)

### Checklist: SSE stream dropping clients

- [ ] Is `SSE_KEEPALIVE_SECS` < reverse proxy timeout?
- [ ] Is the server emitting ping events? (`event: ping` in the stream)
- [ ] Are there `channel lagged` messages in logs? (SSE ring buffer is full)
- [ ] Is `soroban_pulse_sse_active_connections` unusually high, indicating resource exhaustion?
- [ ] Does the client reconnect using `Last-Event-ID`?

### Checklist: Webhook delivery failures

- [ ] Is the subscriber endpoint reachable from the server network?
- [ ] Does `curl -X POST <subscriber_url>` succeed manually?
- [ ] Is the subscriber returning `2xx`?
- [ ] Is HMAC signature verification failing on the subscriber side?
- [ ] Is the payload size within the subscriber's limits?

---

## Escalation Paths

### When to escalate

Escalate immediately for:
- Data loss or suspected data corruption
- Security incident (unexpected admin access, leaked credentials)
- SEV-1 not resolved within 30 minutes
- Indexer split-brain (multiple leaders)

### Internal escalation

| Situation | Contact |
|-----------|---------|
| Database failure | Database / infrastructure team |
| RPC endpoint degradation | Stellar network team / RPC provider |
| Security incident | Security team (immediately, skip normal queue) |
| Persistent SEV-1 | Engineering lead on-call |

### On-call references

- Prometheus alert definitions: [`docs/alerts.yml`](../alerts.yml)
- Alertmanager routing config: [`docs/alertmanager.yml`](../alertmanager.yml)
- PagerDuty escalation policy: configured in PagerDuty under the `SorobanPulse` service

### Declaring an incident

1. Open a new incident channel or war room (e.g., `#incident-YYYY-MM-DD`)
2. Assign an incident commander
3. Post current status: what is failing, impact, steps taken so far
4. Update the channel every 15 minutes until resolved
5. After resolution, write a post-mortem within 48 hours covering timeline, root cause, and follow-up actions

### Useful commands at a glance

```bash
# Service health
curl http://<host>/healthz/ready | jq .

# Current lag
curl http://<host>/metrics | grep soroban_pulse_indexer_lag_ledgers

# Restart the service (Kubernetes)
kubectl rollout restart deployment/soroban-pulse

# Pause indexer
curl -X POST http://<host>/v1/admin/indexer/pause \
  -H "X-Api-Key: $ADMIN_API_KEY"

# Resume indexer
curl -X POST http://<host>/v1/admin/indexer/resume \
  -H "X-Api-Key: $ADMIN_API_KEY"

# View recent logs
kubectl logs -l app=soroban-pulse --tail=200 | grep -i error

# Database active connections
psql $DATABASE_URL -c "SELECT count(*), state FROM pg_stat_activity GROUP BY state;"

# Advisory lock status
psql $DATABASE_URL -c "SELECT pid, granted FROM pg_locks WHERE locktype = 'advisory';"
```

---

## Related Runbooks

| Runbook | Link |
|---------|------|
| Indexer lag | [indexer-lag.md](indexer-lag.md) |
| DB pool exhaustion | [db-pool-exhaustion.md](db-pool-exhaustion.md) |
| RPC errors | [rpc-errors.md](rpc-errors.md) |
| Webhook failures | [webhook-failures.md](webhook-failures.md) |
| SSE connections | [sse-connections.md](sse-connections.md) |
| Notifications | [notifications.md](notifications.md) |
| Feature flag rollback | [feature-flag-rollback.md](feature-flag-rollback.md) |

## Related Documentation

- [Deployment Guide](../deployment.md)
- [Troubleshooting Guide](../troubleshooting.md)
- [Performance Tuning Guide](../performance-tuning.md)
- [Alert Definitions](../alerts.yml)
- [Grafana Dashboard](../grafana-dashboard.json)
