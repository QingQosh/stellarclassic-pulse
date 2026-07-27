# =============================================================================
# Module: rds — Outputs
# =============================================================================

output "db_instance_id" {
  description = "RDS instance identifier."
  value       = aws_db_instance.main.identifier
}

output "db_endpoint" {
  description = "RDS connection endpoint (host:port)."
  value       = "${aws_db_instance.main.address}:${aws_db_instance.main.port}"
  sensitive   = true
}

output "db_name" {
  description = "Name of the PostgreSQL database."
  value       = aws_db_instance.main.db_name
}

output "db_secret_arn" {
  description = "ARN of the Secrets Manager secret holding the DB password."
  value       = aws_secretsmanager_secret.db.arn
  sensitive   = true
}

output "db_resource_id" {
  description = "RDS resource ID (used in IAM policies and metrics)."
  value       = aws_db_instance.main.resource_id
}

output "db_arn" {
  description = "ARN of the RDS instance."
  value       = aws_db_instance.main.arn
}
