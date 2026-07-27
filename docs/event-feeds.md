# Event feeds

`GET /v1/events/feed.rss` returns an Atom XML feed for recent events.

Supported query filters mirror the event list endpoint for common feed-reader
use:

- `contract_id`
- `event_type`
- `from_ledger`
- `to_ledger`
- `limit`

Feeds are cached in-process for 60 seconds per filter set.
