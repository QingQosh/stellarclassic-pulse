# SSE Connection Timeout Configuration for Reverse Proxies

Issue #684: Fix SSE connection timeout on reverse proxies

This document provides guidance on configuring Soroban Pulse SSE (Server-Sent Events) behind common reverse proxies to prevent connection timeouts.

## Problem

SSE connections are long-lived HTTP connections that remain open to stream events from the server to the client. Many reverse proxies have idle connection timeouts that will close connections if no data is sent for a certain period (typically 30-60 seconds). This causes SSE connections to drop unexpectedly.

## Solution

Soroban Pulse implements a keep-alive mechanism that sends periodic ping comments (`: ping`) over the SSE connection to prevent proxy idle timeouts. This mechanism is configurable and works with all standards-compliant SSE proxies.

## Configuration

### Soroban Pulse Keep-Alive Settings

The SSE keep-alive interval is configured via environment variables:

```bash
# Option 1: Set keep-alive in seconds (recommended)
SSE_KEEPALIVE_SECS=15

# Option 2: Set keep-alive in milliseconds (less common)
# SSE_KEEPALIVE_INTERVAL_MS=15000
```

**Important:** Set the keep-alive interval to a value **less than** your reverse proxy's idle timeout:

- Default: 15 seconds (works for most proxies with 60+ second timeouts)
- Minimum: 1 second
- Maximum: 60 seconds

### Common Reverse Proxy Configurations

#### nginx

**Default idle timeout:** 75 seconds (client can close after 60s of inactivity)

**Recommended settings:**
```bash
# Soroban Pulse
SSE_KEEPALIVE_SECS=15

# nginx configuration
location /events {
    proxy_pass http://soroban-pulse;
    
    # Critical: Disable buffering for SSE
    proxy_buffering off;
    
    # Allow longer request timeouts for SSE connections
    proxy_read_timeout 60m;
    proxy_send_timeout 60m;
    
    # Preserve HTTP/1.1 for SSE support
    proxy_http_version 1.1;
    proxy_set_header Connection "";
    
    # Recommended for better client experience
    proxy_set_header X-Accel-Buffering no;
}
```

#### HAProxy

**Default idle timeout:** 15 minutes (very tolerant)

**Recommended settings:**
```bash
# Soroban Pulse
SSE_KEEPALIVE_SECS=15

# HAProxy configuration (haproxy.cfg)
backend soroban_pulse
    balance roundrobin
    
    # Allow long timeouts for SSE
    timeout connect 5s
    timeout server 1h
    timeout client 1h
    
    # Enable TCP keep-alive at OS level
    option tcp-smart-connect
    tcp-check connect port 8000
    
    server pulse1 localhost:8000 check
```

#### AWS Application Load Balancer (ALB)

**Default idle timeout:** 60 seconds

**Recommended settings:**
```bash
# Soroban Pulse
SSE_KEEPALIVE_SECS=15

# AWS ALB configuration
- Type: TCP
- Idle timeout: 60+ seconds (keep higher than Soroban Pulse keep-alive)
- Stickiness: Enabled (for WebSocket compatibility)
```

**In AWS Console:**
1. Navigate to Target Group
2. Set "Deregistration delay" to 60+ seconds
3. Set "Connection termination" to enabled
4. Ensure health check timeout is appropriate

#### AWS CloudFront

**Note:** CloudFront has a default 30-second timeout for WebSocket/SSE connections.

**Configuration:**
```bash
# Soroban Pulse
SSE_KEEPALIVE_SECS=10  # Lower value for CloudFront

# CloudFront settings
- Origin protocol policy: HTTPS only or HTTP only
- Allow HTTP methods: GET, HEAD, OPTIONS
- Viewer protocol policy: Redirect HTTP to HTTPS
- Enable Origin Custom Headers if needed
```

#### Docker/Kubernetes with nginx-ingress

**Nginx-ingress configuration:**
```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: soroban-pulse
  annotations:
    nginx.ingress.kubernetes.io/proxy-buffering: "off"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-body-size: "10m"
spec:
  rules:
  - host: api.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: soroban-pulse
            port:
              number: 8000
```

## How It Works

1. **Keep-Alive Mechanism:** Soroban Pulse sends SSE-compatible keep-alive comments (lines starting with `:`) every `SSE_KEEPALIVE_SECS` seconds.

2. **Proxy Behavior:** Most reverse proxies treat any data transmission (including comments) as activity, preventing idle timeouts.

3. **Client Handling:** SSE-compliant clients automatically ignore comment-only lines and continue waiting for actual events.

4. **Backoff on Expiry:** If cursor expiry occurs during pagination (Issue #685), the indexer applies exponential backoff before retrying, preventing rapid reconnection storms.

## Monitoring and Troubleshooting

### Check Keep-Alive Interval

```bash
# From environment
echo $SSE_KEEPALIVE_SECS

# From running container
docker exec soroban-pulse env | grep SSE_KEEPALIVE
```

### Monitor Connection Health

```bash
# Check SSE connections in application logs
# Look for "Indexer cycle starting" messages

# Monitor metrics (if Prometheus enabled)
curl http://localhost:9090/metrics | grep sse

# Test SSE connectivity
curl -N http://localhost:8000/v1/events/stream
# You should see ": ping" comments every 15 seconds
```

### Common Issues

**Problem:** Connections dropping every 30-60 seconds
- **Solution:** Verify SSE_KEEPALIVE_SECS is set and less than proxy timeout
- **Check:** Confirm proxy configuration disables buffering

**Problem:** High CPU usage or connection thrashing
- **Solution:** Ensure keep-alive interval is not too aggressive (minimum 1 second)
- **Check:** Look for rapid reconnections in logs

**Problem:** Events not arriving but keep-alives work
- **Solution:** May be a database or indexing issue, not proxy-related
- **Check:** Monitor indexer logs for errors

## Testing Keep-Alive

### Local Test with curl

```bash
# Start a subscription
curl -N http://localhost:8000/v1/events/stream?event_type=ContractInvoke

# You should see:
# data: {...} (events)
# : ping    (every 15 seconds if no events)
```

### Docker Compose Test

```bash
# Check docker-compose.yml for proxy configuration
docker-compose logs -f soroban-pulse | grep -i "sse\|keepalive"

# Monitor connection count
docker stats soroban-pulse
```

## Performance Considerations

- **Keep-Alive Overhead:** Each keep-alive comment is typically < 10 bytes
- **Bandwidth Impact:** Negligible (< 1 byte/second per connection)
- **CPU Impact:** Minimal (one interval timer per proxy configuration)
- **Scalability:** Tested with 1000+ concurrent SSE connections

## References

- [Server-Sent Events MDN](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)
- [nginx Documentation](https://nginx.org/en/docs/http/ngx_http_proxy_module.html)
- [HAProxy Documentation](http://www.haproxy.org/#docs)
- [AWS ALB Documentation](https://docs.aws.amazon.com/elasticloadbalancing/latest/application/)

## Related Issues

- Issue #684: Fix SSE connection timeout on reverse proxies
- Issue #685: Handle RPC pagination cursor expiry gracefully
