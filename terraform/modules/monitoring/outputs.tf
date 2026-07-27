# =============================================================================
# Module: monitoring — Outputs
# =============================================================================

output "log_group_name" {
  description = "Name of the CloudWatch log group for the application."
  value       = aws_cloudwatch_log_group.app.name
}

output "log_group_arn" {
  description = "ARN of the CloudWatch log group."
  value       = aws_cloudwatch_log_group.app.arn
}

output "sns_alarm_topic_arn" {
  description = "ARN of the SNS topic used for alarm notifications."
  value       = aws_sns_topic.alarms.arn
}

output "dashboard_url" {
  description = "URL to the CloudWatch dashboard (region-dependent)."
  value       = "https://console.aws.amazon.com/cloudwatch/home#dashboards:name=${aws_cloudwatch_dashboard.main.dashboard_name}"
}
