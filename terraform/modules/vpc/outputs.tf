# =============================================================================
# Module: vpc — Outputs
# =============================================================================

output "vpc_id" {
  description = "ID of the VPC."
  value       = aws_vpc.main.id
}

output "public_subnet_ids" {
  description = "IDs of the public subnets."
  value       = aws_subnet.public[*].id
}

output "private_subnet_ids" {
  description = "IDs of the private subnets."
  value       = aws_subnet.private[*].id
}

output "alb_security_group_id" {
  description = "Security group ID for the Application Load Balancer."
  value       = aws_security_group.alb.id
}

output "app_security_group_id" {
  description = "Security group ID for the application layer."
  value       = aws_security_group.app.id
}

output "rds_security_group_id" {
  description = "Security group ID for the RDS instance."
  value       = aws_security_group.rds.id
}

output "nat_gateway_ids" {
  description = "IDs of the NAT Gateways (empty if NAT is disabled)."
  value       = aws_nat_gateway.main[*].id
}
