# =============================================================================
# Module: monitoring — Variables
# =============================================================================

variable "name_prefix" {
  description = "Prefix applied to every resource name."
  type        = string
}

variable "rds_instance_id" {
  description = "RDS DB instance identifier."
  type        = string
}

variable "alb_arn_suffix" {
  description = "ALB ARN suffix for CloudWatch metric dimensions."
  type        = string
}

variable "target_group_arn_suffix" {
  description = "Target group ARN suffix for CloudWatch metric dimensions."
  type        = string
}

variable "alarm_actions_arn" {
  description = "SNS topic ARNs to notify when alarms fire."
  type        = list(string)
  default     = []
}

variable "ok_actions_arn" {
  description = "SNS topic ARNs to notify when alarms recover."
  type        = list(string)
  default     = []
}

variable "rds_cpu_alarm_threshold" {
  description = "CPU % that triggers an RDS alarm."
  type        = number
  default     = 80
}

variable "rds_storage_alarm_threshold" {
  description = "Free storage bytes below which an RDS alarm fires."
  type        = number
  default     = 5368709120
}

variable "alb_5xx_alarm_threshold" {
  description = "5xx count per minute that triggers an ALB alarm."
  type        = number
  default     = 10
}

variable "alb_latency_alarm_threshold" {
  description = "p99 latency in seconds that triggers an ALB alarm."
  type        = number
  default     = 1.0
}

variable "log_retention_days" {
  description = "CloudWatch log retention period in days."
  type        = number
  default     = 30
}
