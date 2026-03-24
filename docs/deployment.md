# Deployment Guide

## TLS Termination

Soroban Pulse speaks plain HTTP and **must never be exposed directly on port 80 or 443 without TLS in front of it**. All TLS termination must happen at a reverse proxy or load balancer layer.

Set `BEHIND_PROXY=true` in your environment so the service trusts `X-Forwarded-For` headers from the proxy and logs real client IPs.

---

### Option 1 — nginx (self-managed)

Install certbot and obtain a certificate, then use the config below.

```nginx
# /etc/nginx/sites-available/soroban-pulse
server {
    listen 80;
    server_name api.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name api.example.com;

    ssl_certificate     /etc/letsencrypt/live/api.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.example.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;

    location / {
        proxy_pass         http://127.0.0.1:3000;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }
}
```

```bash
# Enable and reload
sudo ln -s /etc/nginx/sites-available/soroban-pulse /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

---

### Option 2 — Caddy (automatic HTTPS)

Caddy obtains and renews Let's Encrypt certificates automatically.

```caddyfile
# /etc/caddy/Caddyfile
api.example.com {
    reverse_proxy localhost:3000
}
```

```bash
sudo systemctl reload caddy
```

---

### Option 3 — AWS Application Load Balancer (ALB)

1. Create an ALB with an HTTPS listener on port 443.
2. Attach an ACM certificate to the listener.
3. Add a target group pointing to the EC2/ECS instance on port 3000.
4. Set the security group to allow inbound 443 from the internet and inbound 3000 **only from the ALB security group** — never from `0.0.0.0/0`.
5. Set `BEHIND_PROXY=true` on the service so ALB-injected `X-Forwarded-For` headers are trusted.

---

## Environment Variables

| Variable        | Description                                              | Default |
|-----------------|----------------------------------------------------------|---------|
| `BEHIND_PROXY`  | Trust `X-Forwarded-For` from upstream proxy/load balancer | `false` |

See the root [README](../README.md) for all other variables.

---

## Security Checklist

- [ ] TLS termination is handled by nginx, Caddy, or a cloud load balancer
- [ ] Port 3000 is firewalled from public internet access
- [ ] `BEHIND_PROXY=true` is set when running behind a proxy
- [ ] Certificates are auto-renewed (certbot timer or Caddy/ACM managed)
