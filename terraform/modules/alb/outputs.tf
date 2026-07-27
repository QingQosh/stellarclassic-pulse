# =============================================================================
# Module: alb — Outputs
# =============================================================================

output "alb_arn" {
  description = "ARN of the Application Load Balancer."
  value       = aws_lb.main.arn
}

output "alb_arn_suffix" {
  description = "Suffix used in CloudWatch metrics for the ALB."
  value       = aws_lb.main.arn_suffix
}

output "alb_dns_name" {
  description = "DNS name of the ALB."
  value       = aws_lb.main.dns_name
}

output "alb_zone_id" {
  description = "Route53 hosted zone ID of the ALB (for alias records)."
  value       = aws_lb.main.zone_id
}

output "target_group_arn" {
  description = "ARN of the target group."
  value       = aws_lb_target_group.app.arn
}

output "target_group_arn_suffix" {
  description = "Suffix used in CloudWatch metrics for the target group."
  value       = aws_lb_target_group.app.arn_suffix
}

output "http_listener_arn" {
  description = "ARN of the HTTP → HTTPS redirect listener."
  value       = aws_lb_listener.http.arn
}

output "https_listener_arn" {
  description = "ARN of the HTTPS listener."
  value       = aws_lb_listener.https.arn
}
