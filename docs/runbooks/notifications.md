# Notification Delivery Troubleshooting

## NotificationDeliverySLABreach

**Alert**: Notification delivery p95 latency exceeds 30s SLA
**Severity**: Critical

### Symptoms
- Notifications arriving late to users
- P95 delivery latency above 30 seconds
- Potential user-facing impact on alert delivery

### Investigation Steps

1. **Check notification queue depth**:
   ```sql
   SELECT COUNT(*) FROM notifications WHERE status = 'pending';
   SELECT COUNT(*) FROM notifications WHERE status = 'queued';
   ```

2. **Check notification pipeline metrics**:
   ```bash
   curl -s http://localhost:3000/metrics | grep notification
   ```

3. **Review downstream service health**:
   - Email provider status
   - Webhook endpoint availability
   - Slack/PagerDuty API status

### Common Causes

1. **High notification volume**: Spike in events causing queue backup
2. **Downstream service outage**: Email or webhook service unavailable
3. **Rate limiting**: External API rate limits being hit
4. **Database contention**: Lock contention on notification table

### Resolution

1. **Immediate**: Scale up notification workers
   ```bash
   kubectl scale deployment soroban-pulse-notifications --replicas=5
   ```

2. **Short-term**: Increase worker concurrency
   ```bash
   kubectl set env deployment/soroban-pulse NOTIFICATION_WORKERS=10
   ```

3. **Long-term**: Review notification batching strategy

### Runbook References
- [Email Notifications](../email-notifications.md)
- [Notification Channels](../notification-channels.md)
- [Rate Limiting](../notification-rate-limiting.md)

---

## NotificationDeliveryLatencyHigh

**Alert**: Notification delivery p99 latency exceeds 60s
**Severity**: Warning

### Symptoms
- Occasional delayed notifications
- P99 latency approaching SLA breach

### Investigation Steps

1. **Check for slow consumers**:
   ```sql
   SELECT channel, AVG(delivery_time) 
   FROM notification_delivery_log 
   WHERE created_at > NOW() - INTERVAL '1 hour'
   GROUP BY channel;
   ```

2. **Review webhook endpoint response times**

3. **Check for network issues**

### Resolution
- Optimize slow webhook endpoints
- Consider async processing for heavy payloads
- Implement circuit breakers for unreliable downstream services

---

## Common Remediation Commands

### Flush stuck notifications
```sql
UPDATE notifications SET status = 'pending' 
WHERE status = 'queued' AND created_at < NOW() - INTERVAL '1 hour';
```

### Check notification worker health
```bash
kubectl get pods -l app=soroban-pulse -o jsonpath='{.items[*].status.conditions[?(@.type=="Ready")].status}'
```

### Restart notification workers
```bash
kubectl rollout restart deployment/soroban-pulse-notifications
```
