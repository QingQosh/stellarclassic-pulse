-- Create Telegram integration configuration table
CREATE TABLE IF NOT EXISTS telegram_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    bot_token TEXT NOT NULL,
    chat_id VARCHAR(255) NOT NULL,
    webhook_enabled BOOLEAN DEFAULT FALSE,
    webhook_url TEXT,
    message_thread_support BOOLEAN DEFAULT FALSE,
    button_support BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(subscription_id)
);

-- Create table for tracking sent Telegram messages
CREATE TABLE IF NOT EXISTS telegram_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    telegram_integration_id UUID NOT NULL REFERENCES telegram_integrations(id) ON DELETE CASCADE,
    event_id UUID,
    message_id VARCHAR(255) NOT NULL,
    chat_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create table for Telegram user subscriptions
CREATE TABLE IF NOT EXISTS telegram_user_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    telegram_integration_id UUID NOT NULL REFERENCES telegram_integrations(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    username VARCHAR(255),
    chat_id VARCHAR(255) NOT NULL,
    subscribed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(telegram_integration_id, user_id)
);

-- Create index for efficient lookups
CREATE INDEX idx_telegram_integrations_subscription ON telegram_integrations(subscription_id);
CREATE INDEX idx_telegram_messages_integration ON telegram_messages(telegram_integration_id);
CREATE INDEX idx_telegram_messages_event ON telegram_messages(event_id);
CREATE INDEX idx_telegram_user_subscriptions_integration ON telegram_user_subscriptions(telegram_integration_id);
