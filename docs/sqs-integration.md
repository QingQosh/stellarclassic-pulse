# AWS SQS integration

Enable the `sqs` feature and set:

- `AWS_REGION`
- `SQS_QUEUE_URL`
- optional `SQS_DLQ_URL`
- optional `SQS_BATCH_SIZE` from 1 to 10

The publisher signs requests with AWS SigV4 using `AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, and optional `AWS_SESSION_TOKEN`. Messages are
serialized Soroban events. Failed batch entries are copied to `SQS_DLQ_URL`
when configured.
