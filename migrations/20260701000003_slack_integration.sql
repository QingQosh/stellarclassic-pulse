-- Create Slack integration configuration table
CREATE TABLE IF NOT EXISTS slack_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    webhook_url TEXT,
    bot_token TEXT,
    channel VARCHAR(255) NOT NULL,
    app_id VARCHAR(255),
    signing_secret TEXT,
    block_kit_enabled BOOLEAN DEFAULT TRUE,
    thread_support BOOLEAN DEFAULT FALSE,
    user_mentions_enabled BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(subscription_id)
);

-- Create table for tracking sent Slack messages
CREATE TABLE IF NOT EXISTS slack_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slack_integration_id UUID NOT NULL REFERENCES slack_integrations(id) ON DELETE CASCADE,
    event_id UUID,
    message_ts TEXT NOT NULL,
    channel_id TEXT,
    thread_ts TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create table for Slack user subscriptions
CREATE TABLE IF NOT EXISTS slack_user_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slack_integration_id UUID NOT NULL REFERENCES slack_integrations(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    user_email VARCHAR(255),
    subscribed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(slack_integration_id, user_id)
);

-- Create index for efficient lookups
CREATE INDEX idx_slack_integrations_subscription ON slack_integrations(subscription_id);
CREATE INDEX idx_slack_messages_integration ON slack_messages(slack_integration_id);
CREATE INDEX idx_slack_messages_event ON slack_messages(event_id);
CREATE INDEX idx_slack_user_subscriptions_integration ON slack_user_subscriptions(slack_integration_id);
