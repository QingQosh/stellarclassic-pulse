# =============================================================================
# Module: alb — Variables
# =============================================================================

variable "name_prefix" {
  description = "Prefix applied to every resource name."
  type        = string
}

variable "vpc_id" {
  description = "ID of the VPC."
  type        = string
}

variable "public_subnet_ids" {
  description = "IDs of public subnets where the ALB is placed."
  type        = list(string)
}

variable "alb_security_group_id" {
  description = "Security group ID to attach to the ALB."
  type        = string
  default     = ""
}

variable "internal" {
  description = "Create an internal (VPC-only) ALB."
  type        = bool
  default     = false
}

variable "access_logs_enabled" {
  description = "Enable ALB access logs to S3."
  type        = bool
  default     = false
}

variable "idle_timeout" {
  description = "ALB idle connection timeout in seconds."
  type        = number
  default     = 60
}

variable "certificate_arn" {
  description = "ARN of the ACM certificate for the HTTPS listener."
  type        = string
}

variable "health_check_path" {
  description = "HTTP path for target group health checks."
  type        = string
  default     = "/healthz/ready"
}

variable "health_check_interval" {
  description = "Seconds between health checks."
  type        = number
  default     = 30
}

variable "health_check_threshold" {
  description = "Consecutive successes to mark a target healthy."
  type        = number
  default     = 2
}

variable "app_port" {
  description = "Port the application containers listen on."
  type        = number
  default     = 3000
}
