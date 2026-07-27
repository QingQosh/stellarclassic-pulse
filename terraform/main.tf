# =============================================================================
# Root Module — SorobanPulse Infrastructure (Issue #650)
#
# Wires together:
#   - VPC + subnets + NAT gateway
#   - RDS PostgreSQL
#   - Application Load Balancer
#   - Monitoring / CloudWatch alarms
# =============================================================================

locals {
  name_prefix = "${var.project_name}-${var.environment}"
}

# ---------------------------------------------------------------------------
# Networking
# ---------------------------------------------------------------------------

module "vpc" {
  source = "./modules/vpc"

  name_prefix          = local.name_prefix
  vpc_cidr             = var.vpc_cidr
  availability_zones   = var.availability_zones
  public_subnet_cidrs  = var.public_subnet_cidrs
  private_subnet_cidrs = var.private_subnet_cidrs
  enable_nat_gateway   = var.enable_nat_gateway
  single_nat_gateway   = var.single_nat_gateway

  tags = {
    Environment = var.environment
  }
}

# ---------------------------------------------------------------------------
# RDS PostgreSQL
# ---------------------------------------------------------------------------

module "rds" {
  source = "./modules/rds"

  name_prefix                     = local.name_prefix
  vpc_id                          = module.vpc.vpc_id
  private_subnet_ids              = module.vpc.private_subnet_ids
  allowed_security_group_ids      = [module.vpc.app_security_group_id]
  rds_security_group_id           = module.vpc.rds_security_group_id
  db_name                         = var.db_name
  db_username                     = var.db_username
  db_instance_class               = var.db_instance_class
  db_engine_version               = var.db_engine_version
  db_allocated_storage            = var.db_allocated_storage
  db_max_allocated_storage        = var.db_max_allocated_storage
  db_backup_retention_period      = var.db_backup_retention_period
  db_multi_az                     = var.db_multi_az
  db_deletion_protection          = var.db_deletion_protection
  db_apply_immediately            = var.db_apply_immediately
  db_skip_final_snapshot          = var.db_skip_final_snapshot
  db_performance_insights_enabled = var.db_performance_insights_enabled
  db_monitoring_interval          = var.db_monitoring_interval
}

# ---------------------------------------------------------------------------
# Application Load Balancer
# ---------------------------------------------------------------------------

module "alb" {
  source = "./modules/alb"

  name_prefix            = local.name_prefix
  vpc_id                 = module.vpc.vpc_id
  public_subnet_ids      = module.vpc.public_subnet_ids
  alb_security_group_id  = module.vpc.alb_security_group_id
  internal               = var.alb_internal
  access_logs_enabled    = var.alb_access_logs_enabled
  idle_timeout           = var.alb_idle_timeout
  certificate_arn        = var.certificate_arn
  health_check_path      = var.health_check_path
  health_check_interval  = var.health_check_interval
  health_check_threshold = var.health_check_threshold
  app_port               = var.app_port
}

# ---------------------------------------------------------------------------
# Monitoring & CloudWatch alarms
# ---------------------------------------------------------------------------

module "monitoring" {
  source = "./modules/monitoring"

  name_prefix                 = local.name_prefix
  rds_instance_id             = module.rds.db_instance_id
  alb_arn_suffix              = module.alb.alb_arn_suffix
  target_group_arn_suffix     = module.alb.target_group_arn_suffix
  alarm_actions_arn           = var.alarm_actions_arn
  ok_actions_arn              = var.ok_actions_arn
  rds_cpu_alarm_threshold     = var.rds_cpu_alarm_threshold
  rds_storage_alarm_threshold = var.rds_storage_alarm_threshold
  alb_5xx_alarm_threshold     = var.alb_5xx_alarm_threshold
  alb_latency_alarm_threshold = var.alb_latency_alarm_threshold
  log_retention_days          = var.log_retention_days
}
