# GraphQL API Documentation

Issue #683: Add GraphQL API layer for more flexible event querying

## Overview

Soroban Pulse provides a GraphQL API endpoint for flexible querying of Soroban contract events. GraphQL offers several advantages over REST:

- **Flexible querying**: Request only the fields you need
- **Single request**: Get related data in one query instead of multiple REST calls
- **Real-time subscriptions**: Subscribe to live events via WebSocket
- **Introspection**: Discover the API schema directly
- **Powerful filtering**: Complex filters without URL parameter encoding

## Quick Start

### Accessing the GraphQL Playground

Open your browser and navigate to:
```
http://localhost:8000/graphql
```

This opens the GraphQL Playground IDE where you can:
- Write and test queries
- Explore the schema
- View real-time documentation
- See query results immediately

### Making GraphQL Requests

#### Using curl

```bash
curl -X POST http://localhost:8000/graphql \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: your-api-key" \
  -d '{"query":"query { events(first: 10) { id contractId eventType } }"}'
```

#### Using GraphQL Client Libraries

**JavaScript/TypeScript:**
```bash
npm install @apollo/client graphql
```

```typescript
import { ApolloClient, InMemoryCache, HttpLink, gql } from '@apollo/client';

const client = new ApolloClient({
  link: new HttpLink({
    uri: 'http://localhost:8000/graphql',
    headers: {
      'X-Api-Key': 'your-api-key',
    },
  }),
  cache: new InMemoryCache(),
});

const query = gql`
  query GetEvents {
    events(first: 10) {
      id
      contractId
      eventType
      ledger
    }
  }
`;

const result = await client.query({ query });
```

**Python:**
```bash
pip install gql
```

```python
from gql import Client, gql
from gql.transport.requests import RequestsHTTPTransport

transport = RequestsHTTPTransport(
    url="http://localhost:8000/graphql",
    headers={"X-Api-Key": "your-api-key"}
)

client = Client(transport=transport)

query = gql("""
    query GetEvents {
        events(first: 10) {
            id
            contractId
            eventType
            ledger
        }
    }
""")

result = client.execute(query)
```

## Schema Reference

### Query Root

#### `event(id: ID!): Event`

Get a single event by ID.

**Example:**
```graphql
query {
  event(id: "550e8400-e29b-41d4-a716-446655440000") {
    contractId
    eventType
    txHash
    ledger
  }
}
```

#### `events(filter: EventFilter, pagination: PaginationInput): [Event!]!`

Query events with optional filters and pagination.

**Example:**
```graphql
query {
  events(
    filter: {
      contractId: "CABC123"
      eventType: "contract_invoke"
      ledgerMin: 1000
      ledgerMax: 2000
    }
    pagination: { first: 50 }
  ) {
    id
    contractId
    eventType
    ledger
    ledgerCloseTime
    value
  }
}
```

#### `eventStats: EventStats!`

Get statistics about events in the system.

**Example:**
```graphql
query {
  eventStats {
    totalEvents
    eventsByType {
      eventType
      count
    }
    topContracts {
      contractId
      count
    }
  }
}
```

### Types

#### Event

```graphql
type Event {
  id: ID!
  contractId: String!
  eventType: String!
  txHash: String!
  ledger: UInt64!
  ledgerCloseTime: DateTime!
  topic: [String!]
  value: String
  sourceAccount: String
}
```

#### EventFilter

Input filter for querying events.

```graphql
input EventFilter {
  # Filter by contract ID (exact match)
  contractId: String
  
  # Filter by event type (exact match)
  eventType: String
  
  # Filter by transaction hash (exact match)
  txHash: String
  
  # Filter by ledger sequence (exact match)
  ledger: UInt64
  
  # Filter by minimum ledger sequence
  ledgerMin: UInt64
  
  # Filter by maximum ledger sequence
  ledgerMax: UInt64
  
  # Filter by source account (exact match)
  sourceAccount: String
  
  # Filter by topic (partial match)
  topicContains: String
}
```

#### PaginationInput

```graphql
input PaginationInput {
  # Maximum number of results (default: 10, max: 100)
  first: Int
  
  # Cursor for pagination
  after: String
}
```

#### EventStats

```graphql
type EventStats {
  totalEvents: Int!
  eventsByType: [EventTypeCount!]!
  topContracts: [ContractCount!]!
}

type EventTypeCount {
  eventType: String!
  count: Int!
}

type ContractCount {
  contractId: String!
  count: Int!
}
```

### Subscription Root

#### `events(filter: EventFilter): Event!`

Subscribe to new events matching optional filters (WebSocket).

**Example:**
```graphql
subscription {
  events(filter: { contractId: "CABC123" }) {
    id
    contractId
    eventType
    ledger
    value
  }
}
```

## Query Examples

### Find Recent Events for a Contract

```graphql
query {
  events(
    filter: {
      contractId: "CABC123"
      ledgerMin: 1000
    }
    pagination: { first: 20 }
  ) {
    id
    eventType
    ledger
    ledgerCloseTime
    value
  }
}
```

### Get Event Statistics

```graphql
query {
  eventStats {
    totalEvents
    eventsByType {
      eventType
      count
    }
    topContracts(limit: 10) {
      contractId
      count
    }
  }
}
```

### Filter Events by Transaction

```graphql
query {
  events(
    filter: {
      txHash: "a1b2c3d4e5f6..."
    }
  ) {
    id
    contractId
    eventType
    topic
    value
  }
}
```

### Get Events in Ledger Range

```graphql
query {
  events(
    filter: {
      ledgerMin: 5000000
      ledgerMax: 5000100
    }
    pagination: { first: 100 }
  ) {
    id
    contractId
    ledger
    ledgerCloseTime
  }
}
```

## Subscriptions

Subscribe to real-time events using WebSocket:

```graphql
subscription OnNewEvents {
  events(filter: { eventType: "contract_invoke" }) {
    id
    contractId
    eventType
    ledger
    ledgerCloseTime
  }
}
```

### JavaScript WebSocket Example

```typescript
import { createClient } from 'graphql-ws';
import WebSocket from 'ws';

const client = createClient({
  url: 'ws://localhost:8000/graphql/ws',
  connectionParams: {
    'X-Api-Key': 'your-api-key',
  },
  webSocketImpl: WebSocket,
});

client.subscribe(
  {
    query: `
      subscription {
        events(filter: { eventType: "contract_invoke" }) {
          id
          contractId
          eventType
          ledger
        }
      }
    `,
  },
  {
    next: (data) => console.log('New event:', data),
    error: (err) => console.error('Subscription error:', err),
    complete: () => console.log('Subscription closed'),
  }
);
```

## Authentication

GraphQL endpoints use the same authentication as REST endpoints:

```bash
# Using X-Api-Key header
curl -H "X-Api-Key: your-api-key" http://localhost:8000/graphql

# Using Authorization header
curl -H "Authorization: Bearer your-api-key" http://localhost:8000/graphql
```

## Error Handling

GraphQL errors are returned in the response with details:

```json
{
  "errors": [
    {
      "message": "Invalid filter parameter",
      "extensions": {
        "code": "INVALID_ARGUMENT"
      }
    }
  ]
}
```

## Performance Tips

1. **Request only needed fields**: GraphQL allows you to specify exactly what you need
   ```graphql
   # Good: Only request needed fields
   query {
     events { id contractId }
   }
   
   # Avoid: Requesting unnecessary fields
   query {
     events { id contractId eventType txHash topic value sourceAccount }
   }
   ```

2. **Use pagination**: Always paginate large result sets
   ```graphql
   query {
     events(pagination: { first: 50 }) { id }
   }
   ```

3. **Filter early**: Apply filters to reduce result set
   ```graphql
   query {
     events(filter: { contractId: "CA..." }) { id }
   }
   ```

4. **Cache responses**: Use HTTP caching for read-heavy workloads

## Comparing REST vs GraphQL

| Feature | REST | GraphQL |
|---------|------|---------|
| Multiple fields | Multiple requests | Single request |
| Over-fetching | Yes | No |
| Schema discovery | Manual docs | Automatic introspection |
| Real-time updates | Polling | Subscriptions |
| Caching | Built-in | Client-side |
| Flexibility | Fixed structure | Full flexibility |

## Related Issues

- Issue #683: Add GraphQL API layer
- Issue #685: Handle RPC pagination cursor expiry gracefully
