# =============================================================================
# Module: rds — Variables
# =============================================================================

variable "name_prefix" {
  description = "Prefix applied to every resource name."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC."
  type        = string
}

variable "private_subnet_ids" {
  description = "IDs of the private subnets for the DB subnet group."
  type        = list(string)
}

variable "allowed_security_group_ids" {
  description = "Security group IDs that are permitted to connect to RDS on port 5432."
  type        = list(string)
}

variable "rds_security_group_id" {
  description = "Security group ID to attach to the RDS instance (created by the vpc module)."
  type        = string
  default     = ""
}

variable "db_name" {
  description = "Name of the PostgreSQL database."
  type        = string
}

variable "db_username" {
  description = "Master username."
  type        = string
  sensitive   = true
}

variable "db_instance_class" {
  description = "RDS instance class."
  type        = string
}

variable "db_engine_version" {
  description = "PostgreSQL engine version."
  type        = string
}

variable "db_allocated_storage" {
  description = "Initial storage in GiB."
  type        = number
}

variable "db_max_allocated_storage" {
  description = "Maximum autoscaling storage in GiB."
  type        = number
}

variable "db_backup_retention_period" {
  description = "Days of automated backup retention."
  type        = number
}

variable "db_multi_az" {
  description = "Enable Multi-AZ deployment."
  type        = bool
}

variable "db_deletion_protection" {
  description = "Prevent accidental deletion."
  type        = bool
}

variable "db_apply_immediately" {
  description = "Apply modifications immediately."
  type        = bool
}

variable "db_skip_final_snapshot" {
  description = "Skip final snapshot on deletion."
  type        = bool
}

variable "db_performance_insights_enabled" {
  description = "Enable Performance Insights."
  type        = bool
}

variable "db_monitoring_interval" {
  description = "Enhanced Monitoring interval in seconds (0 disables it)."
  type        = number
}
