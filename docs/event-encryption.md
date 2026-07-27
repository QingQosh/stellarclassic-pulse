# Event Encryption at Rest (Issue #704)

SorobanPulse supports AES-256-GCM encryption of sensitive event data before storage (issue #704). Encrypted fields are stored in an encrypted envelope with the plaintext never persisted to the database.

## Architecture

```
Event data (JSON) ──► AES-256-GCM encryption ──► Base64 envelope ──► Database storage
Database retrieval ──► Decrypt with current key ──► Original JSON data
Key rotation: Decrypt with old key ──► Re-encrypt with new key
```

## Features

- **AES-256-GCM Encryption**: Industry-standard authenticated encryption
- **Field-Level Encryption**: Encrypt sensitive event_data before storage
- **Key Rotation**: Seamless rotation from old key to new key with dual-key decryption
- **Backward Compatible**: Non-encrypted values pass through unchanged
- **Query Support**: Encrypted events can be queried (returns decrypted data)

## Configuration

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `EVENT_DATA_ENCRYPTION_KEY` | AES-256 key (hex-encoded 64 chars = 32 bytes) | `aabbccdd...` |
| `EVENT_DATA_ENCRYPTION_KEY_OLD` | Previous key for rotation (optional) | `11223344...` |

### Generate an encryption key

```bash
# Generate a random 256-bit key and encode as hex
openssl rand -hex 32
# Output: a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1

# Set as environment variable
export EVENT_DATA_ENCRYPTION_KEY="a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1"
```

## Enabling Encryption

### Enable at startup

```bash
cargo build --features encryption
EVENT_DATA_ENCRYPTION_KEY="<your-hex-encoded-key>" cargo run
```

### Enable in Docker

```dockerfile
ENV EVENT_DATA_ENCRYPTION_KEY=a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1
```

### Enable in Kubernetes

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: soroban-pulse-encryption
type: Opaque
stringData:
  encryption-key: a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: soroban-pulse
spec:
  template:
    spec:
      containers:
        - name: api
          env:
            - name: EVENT_DATA_ENCRYPTION_KEY
              valueFrom:
                secretKeyRef:
                  name: soroban-pulse-encryption
                  key: encryption-key
```

## Encrypted Storage Format

Encrypted event_data is stored as a JSON envelope:

```json
{
  "encrypted": true,
  "data": "<base64-ciphertext>",
  "nonce": "<base64-12-byte-nonce>"
}
```

- **encrypted**: Flag indicating this is an encrypted envelope
- **data**: Base64-encoded ciphertext (AES-256-GCM)
- **nonce**: Base64-encoded 12-byte nonce (generated randomly for each encryption)

Each encryption generates a unique nonce, ensuring identical plaintexts produce different ciphertexts.

## Key Rotation

### Scenario

You want to rotate from an old key to a new key.

```bash
# Current deployment uses old key
OLD_KEY="11223344556677889900aabbccddeeef00112233445566778899aabbccddee"

# Generate new key
NEW_KEY=$(openssl rand -hex 32)
# NEW_KEY="aabbccddee0011223344556677889900112233445566778899aabbccddeeee"
```

### Rotation Steps

1. **Add new key to configuration**:
   ```bash
   export EVENT_DATA_ENCRYPTION_KEY="$NEW_KEY"
   export EVENT_DATA_ENCRYPTION_KEY_OLD="$OLD_KEY"
   ```

2. **Restart the application**: New events are encrypted with the new key. Existing events encrypted with the old key are still decryptable.

3. **Trigger re-encryption job** (optional, to migrate all events to new key):
   ```bash
   curl -X POST \
     -H "X-Api-Key: admin_key" \
     https://api.example.com/v1/admin/reencrypt
   ```

4. **Monitor re-encryption progress**:
   ```bash
   # Check re-encryption status
   curl -H "X-Api-Key: admin_key" \
     https://api.example.com/v1/admin/reencrypt/status
   ```

5. **Remove old key** (after all events are re-encrypted):
   ```bash
   export EVENT_DATA_ENCRYPTION_KEY="$NEW_KEY"
   # Unset EVENT_DATA_ENCRYPTION_KEY_OLD
   ```

### Automatic Decryption with Key Rotation

During key rotation, the decryption process:
1. Tries to decrypt with the current (new) key
2. If that fails, tries the old key (if configured)
3. Returns the decrypted value or passes through non-encrypted data

This ensures seamless operation during key rotation without downtime.

## API Usage

### Retrieving Encrypted Events

Events are automatically decrypted on retrieval:

```bash
curl -H "X-Api-Key: your_api_key" \
  'https://api.example.com/v1/events?contract_id=CABC123'

# Response includes decrypted event_data:
{
  "id": "123e4567-e89b-12d3-a456-426614174000",
  "contract_id": "CABC123",
  "event_data": {
    "amount": 1000000,
    "recipient": "GABC123"
  },
  "tx_hash": "abc123def456...",
  "ledger": 12345
}
```

### Streaming Encrypted Events

SSE streams automatically decrypt events:

```bash
curl -H "X-Api-Key: your_api_key" \
  'https://api.example.com/v1/events/stream?contract_id=CABC123'

# Each event sent via SSE is automatically decrypted
event: soroban_event
data: {"id":"...","event_data":{"amount":1000000},...}
```

### Exporting Encrypted Events

CSV/JSON exports include decrypted data:

```bash
curl -H "X-Api-Key: your_api_key" \
  'https://api.example.com/v1/events/export?contract_id=CABC123&format=csv' \
  > events.csv

# CSV includes decrypted event_data
```

## Field-Level Encryption Configuration

The encryption applies to the entire `event_data` JSON object. To encrypt specific fields:

1. **Application-level approach**: Transform event_data before storage to nest sensitive fields
2. **Custom middleware**: Add middleware that selectively encrypts fields

Example: Encrypt only the `amount` field:

```rust
// Before storage transformation
let event_data = json!({
  "public_info": "transfer",
  "amount": "1000000"
});

// Transform to encrypt sensitive data
let to_encrypt = json!({ "amount": "1000000" });
let encrypted = crate::encryption::encrypt(&key, &to_encrypt)?;

let transformed = json!({
  "public_info": "transfer",
  "encrypted_amount": encrypted
});

// Store transformed with encryption disabled (or store in encrypted column)
```

## Encrypted Query Support

### Query Limitations

Encrypted event_data cannot be queried by content. To enable content-based queries:

1. **Store unencrypted metadata**: Keep searchable fields unencrypted
   ```json
   {
     "contract_id": "CABC123",  // Unencrypted, searchable
     "event_type": "transfer",   // Unencrypted, searchable
     "event_data": { "encrypted": true, "data": "...", "nonce": "..." }
   }
   ```

2. **Use event_type filtering** instead of event_data filters
   ```bash
   GET /v1/events?contract_id=CABC123&event_type=transfer
   ```

### Querying Encrypted Events

All standard queries work; they operate on the encrypted envelope:

```bash
# Get all events for a contract (decrypted on retrieval)
GET /v1/events?contract_id=CABC123

# Stream events (decrypted on delivery)
GET /v1/events/stream?contract_id=CABC123

# Export events (decrypted in output)
GET /v1/events/export?contract_id=CABC123&format=json
```

## Performance Considerations

- **Encryption overhead**: ~1-2ms per event for AES-256-GCM
- **Decryption overhead**: ~1-2ms per event retrieval
- **Key size**: 32 bytes (256 bits) for AES-256
- **Nonce generation**: Cryptographically secure random per event

## Security Considerations

- **Key Storage**: Store keys in environment variables or secrets manager (not in code)
- **Key Rotation**: Use dual-key decryption during rotation to avoid downtime
- **Database Access**: A compromised database only exposes encrypted data without the key
- **Defense-in-depth**: Combine encryption with multi-tenancy (`tenant_id` filtering) and access controls
- **Nonce Reuse**: Never reuse a nonce with the same key; each encryption generates a unique nonce

## Troubleshooting

### "Encryption feature not enabled"

Build with the encryption feature:
```bash
cargo build --features encryption
```

### "Missing encryption key"

Set the `EVENT_DATA_ENCRYPTION_KEY` environment variable:
```bash
export EVENT_DATA_ENCRYPTION_KEY="your-hex-encoded-32-byte-key"
```

### "Decryption failed"

- Verify the encryption key is correct
- Check that the encrypted envelope format is valid
- Ensure the nonce hasn't been corrupted

### Re-encryption job not starting

- Ensure you're using an admin API key
- Check that both current and old keys are valid hex-encoded 32-byte values
- Verify database connectivity

## Re-encryption API

### Start Re-encryption Job

**Endpoint**: `POST /v1/admin/reencrypt`

**Authentication**: Required (admin key)

**Request**:
```bash
curl -X POST \
  -H "X-Api-Key: admin_key" \
  -H "Content-Type: application/json" \
  -d '{"batch_size": 1000}' \
  https://api.example.com/v1/admin/reencrypt
```

**Response** (202 Accepted):
```json
{
  "message": "re-encryption job started",
  "status": "running"
}
```

### Get Re-encryption Status

**Endpoint**: `GET /v1/admin/reencrypt/status`

**Authentication**: Required (admin key)

**Response**:
```json
{
  "is_running": true,
  "events_processed": 50000,
  "events_remaining": 150000,
  "started_at": "2026-03-14T10:00:00Z",
  "progress_percent": 25
}
```

## See Also

- [Multi-Tenancy Deployment](./multi-tenancy.md)
- [API Authentication](./api_authentication.md)
- [Security Guidelines](./security.md)
