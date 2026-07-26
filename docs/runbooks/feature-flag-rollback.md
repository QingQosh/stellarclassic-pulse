# Feature Flag Rollback Troubleshooting

## FeatureFlagAutoRollback

**Alert**: Feature flag was automatically rolled back
**Severity**: Warning

### Symptoms
- Feature flag automatically disabled due to error rate spike
- Users may experience degraded functionality
- Automatic recovery triggered

### Investigation Steps

1. **Check feature flag status**:
   ```sql
   SELECT name, enabled, auto_rollback, error_rate, last_rollback_at 
   FROM feature_flags 
   WHERE auto_rollback = true;
   ```

2. **Review audit log**:
   ```sql
   SELECT * FROM feature_flag_audit 
   WHERE action = 'auto_rollback' 
   ORDER BY created_at DESC 
   LIMIT 10;
   ```

3. **Check error metrics**:
   ```bash
   curl -s http://localhost:3000/metrics | grep feature_flag_error_rate
   ```

### Common Causes

1. **Downstream service degradation**: External dependency failure
2. **Resource exhaustion**: Memory or CPU limits hit
3. **Code regression**: Bug introduced in feature implementation
4. **Traffic spike**: Unexpected load on feature-specific code paths

### Resolution

1. **Immediate**: Verify system stability
   ```bash
   kubectl get pods -l app=soroban-pulse
   kubectl top pods
   ```

2. **Investigate root cause**:
   - Review application logs around rollback time
   - Check for related alerts
   - Review recent deployments

3. **Re-enable flag (if safe)**:
   ```sql
   UPDATE feature_flags 
   SET enabled = true, auto_rollback = false 
   WHERE name = 'flag_name';
   ```
   **Note**: Only re-enable after root cause is identified and fixed.

### Prevention

1. **Gradual rollout**: Use percentage-based rollouts
2. **Monitoring**: Ensure comprehensive error tracking
3. **Circuit breakers**: Implement graceful degradation
4. **Load testing**: Test features under expected load

---

## HighErrorRateRollbackRisk

**Alert**: Error rate approaching feature flag rollback threshold
**Severity**: Warning

### Symptoms
- Error rate elevated but below auto-rollback threshold
- Potential for automatic rollback if trends continue

### Investigation Steps

1. **Identify error sources**:
   ```sql
   SELECT error_type, COUNT(*) 
   FROM error_logs 
   WHERE timestamp > NOW() - INTERVAL '5 minutes'
   GROUP BY error_type 
   ORDER BY COUNT(*) DESC;
   ```

2. **Check feature flag metrics**:
   - Current error rate vs threshold
   - Request volume changes
   - Response time degradation

### Resolution

1. **Monitor closely**: Watch for further degradation
2. **Prepare rollback plan**: Have manual rollback command ready
3. **Investigate proactively**: Don't wait for auto-rollback
4. **Consider manual rollback** if degradation continues

---

## Manual Rollback Procedure

### 1. Identify affected flag
```sql
SELECT name, enabled, error_rate 
FROM feature_flags 
WHERE error_rate > 0.03;
```

### 2. Disable flag
```sql
UPDATE feature_flags 
SET enabled = false, updated_at = NOW() 
WHERE name = 'flag_name';
```

### 3. Log the action
```sql
INSERT INTO feature_flag_audit (flag_name, action, performed_by, reason, created_at)
VALUES ('flag_name', 'manual_rollback', 'ops-team', 'High error rate detected', NOW());
```

### 4. Verify rollback
```bash
curl -s http://localhost:3000/metrics | grep feature_flag_error_rate
```

### 5. Notify stakeholders
Post in #soroban-pulse-incidents with:
- Which flag was rolled back
- Why it was rolled back
- Current system status
- Next steps
