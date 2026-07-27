-- Create Discord integration configuration table
CREATE TABLE IF NOT EXISTS discord_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    webhook_url TEXT NOT NULL,
    bot_name VARCHAR(255),
    avatar_url TEXT,
    embed_enabled BOOLEAN DEFAULT TRUE,
    thread_support BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(subscription_id)
);

-- Create table for tracking sent Discord messages
CREATE TABLE IF NOT EXISTS discord_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    discord_integration_id UUID NOT NULL REFERENCES discord_integrations(id) ON DELETE CASCADE,
    event_id UUID,
    message_id TEXT NOT NULL,
    channel_id TEXT,
    thread_id TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create index for efficient lookups
CREATE INDEX idx_discord_integrations_subscription ON discord_integrations(subscription_id);
CREATE INDEX idx_discord_messages_integration ON discord_messages(discord_integration_id);
CREATE INDEX idx_discord_messages_event ON discord_messages(event_id);
