# Azure Event Hubs integration

Set:

- `EVENT_HUBS_CONNECTION_STRING` with Send permission
- `EVENT_HUB_NAME`, unless `EntityPath` is present in the connection string
- optional `EVENT_HUBS_PARTITION_STRATEGY`: `contract_id`, `tx_hash`, or `ledger`

The publisher uses a pooled HTTPS client and Event Hubs SAS authorization. Events
are serialized as JSON and sent to the Event Hubs `/messages` endpoint.
