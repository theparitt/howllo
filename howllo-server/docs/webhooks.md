# Webhooks

Webhooks deliver real-time events to external HTTP endpoints.

## Endpoints

### Create Webhook

`POST /api/admin/webhooks`

```json
{
    "tenant_slug": "acme",
    "url": "https://example.com/webhook",
    "secret": "optional-secret-for-hmac"
}
```

### List Webhooks

`GET /api/admin/webhooks?tenant_slug=acme`

Returns webhook list. Secret values are never exposed — instead `has_secret: true/false`.

### Deactivate Webhook

`PATCH /api/admin/webhooks/{webhook_id}/deactivate`

Stops webhook delivery. Deactivated webhooks are not included in active deliveries.

### Delivery Status

`GET /api/admin/webhooks/deliveries?tenant_slug=acme`

Returns recent delivery records with attempt count and retry information.

### Resend Delivery

`POST /api/admin/webhooks/deliveries/{delivery_id}/resend`

Manually resends a failed delivery.

## Delivery Headers

Every webhook delivery includes:

| Header | Value |
|--------|-------|
| `x-howllo-event-id` | UUID of the event |
| `x-howllo-timestamp` | Unix timestamp of delivery |
| `x-howllo-signature` | HMAC SHA-256 signature (if secret provided) |

## Signature Verification

```python
import hmac, hashlib

def verify_signature(secret, event_id, timestamp, body):
    expected = hmac.new(
        secret.encode(),
        f"{event_id}.{timestamp}.{body}".encode(),
        hashlib.sha256
    ).hexdigest()
    return f"sha256={expected}"
```

## Event Types

| Event | Trigger |
|-------|---------|
| `post.status_changed` | Post status updated |
| `comment.official_response` | Comment marked as official |

## Timeout

Configure delivery timeout via `HOWLLO_WEBHOOK_TIMEOUT_MS` (default 5000ms).

## Retry

Failed deliveries are recorded with `attempt_count`. Retry scheduling with `next_retry_at` supports exponential backoff. After max attempts, deliveries are marked abandoned.
