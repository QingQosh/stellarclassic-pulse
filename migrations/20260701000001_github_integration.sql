-- Create GitHub integration configuration table
CREATE TABLE IF NOT EXISTS github_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    access_token TEXT NOT NULL,
    owner VARCHAR(255) NOT NULL,
    repository VARCHAR(255) NOT NULL,
    issue_title_template TEXT,
    issue_body_template TEXT,
    auto_create_issues BOOLEAN DEFAULT TRUE,
    pr_comment_enabled BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(subscription_id)
);

-- Create table for tracking created GitHub issues
CREATE TABLE IF NOT EXISTS github_issues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    github_integration_id UUID NOT NULL REFERENCES github_integrations(id) ON DELETE CASCADE,
    event_id UUID,
    issue_number INTEGER NOT NULL,
    repository VARCHAR(255) NOT NULL,
    owner VARCHAR(255) NOT NULL,
    issue_url TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create index for efficient lookups
CREATE INDEX idx_github_integrations_subscription ON github_integrations(subscription_id);
CREATE INDEX idx_github_issues_integration ON github_issues(github_integration_id);
CREATE INDEX idx_github_issues_event ON github_issues(event_id);
