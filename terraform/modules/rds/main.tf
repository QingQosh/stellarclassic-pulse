# =============================================================================
# Module: rds — PostgreSQL on RDS (Issue #650)
#
# Creates:
#   - DB subnet group
#   - DB parameter group (PostgreSQL 16 tuning)
#   - IAM role for Enhanced Monitoring
#   - Secrets Manager secret (auto-generated password)
#   - RDS PostgreSQL instance
# =============================================================================

# ---------------------------------------------------------------------------
# Random password (stored in Secrets Manager — never in state plaintext)
# ---------------------------------------------------------------------------

resource "random_password" "db" {
  length           = 32
  special          = true
  override_special = "!#$%&*()-_=+[]{}<>:?"
}

# ---------------------------------------------------------------------------
# Secrets Manager: DB credentials
# ---------------------------------------------------------------------------

resource "aws_secretsmanager_secret" "db" {
  name                    = "${var.name_prefix}/rds/credentials"
  description             = "SorobanPulse RDS master credentials for ${var.name_prefix}."
  recovery_window_in_days = 7

  tags = {
    Name = "${var.name_prefix}-rds-secret"
  }
}

resource "aws_secretsmanager_secret_version" "db" {
  secret_id = aws_secretsmanager_secret.db.id
  secret_string = jsonencode({
    username = var.db_username
    password = random_password.db.result
    dbname   = var.db_name
    host     = aws_db_instance.main.address
    port     = aws_db_instance.main.port
  })

  # The secret version references the instance, so it must be created after it.
  depends_on = [aws_db_instance.main]
}

# ---------------------------------------------------------------------------
# DB Subnet Group
# ---------------------------------------------------------------------------

resource "aws_db_subnet_group" "main" {
  name        = "${var.name_prefix}-db-subnet-group"
  description = "Private subnets for ${var.name_prefix} RDS."
  subnet_ids  = var.private_subnet_ids

  tags = {
    Name = "${var.name_prefix}-db-subnet-group"
  }
}

# ---------------------------------------------------------------------------
# DB Parameter Group (PostgreSQL 16 tuning)
# ---------------------------------------------------------------------------

resource "aws_db_parameter_group" "main" {
  name        = "${var.name_prefix}-pg16"
  family      = "postgres16"
  description = "Custom parameter group for ${var.name_prefix} PostgreSQL 16."

  # Enable SSL connections only
  parameter {
    name  = "rds.force_ssl"
    value = "1"
  }

  # Log slow queries to help identify missing indexes
  parameter {
    name  = "log_min_duration_statement"
    value = "1000" # 1 second
  }

  parameter {
    name  = "log_connections"
    value = "1"
  }

  parameter {
    name  = "log_disconnections"
    value = "1"
  }

  # Tune autovacuum for write-heavy workloads
  parameter {
    name  = "autovacuum_vacuum_scale_factor"
    value = "0.02"
  }

  parameter {
    name  = "autovacuum_analyze_scale_factor"
    value = "0.01"
  }

  # Use UTC so timestamps are unambiguous
  parameter {
    name  = "timezone"
    value = "UTC"
  }

  tags = {
    Name = "${var.name_prefix}-pg16-params"
  }

  lifecycle {
    create_before_destroy = true
  }
}

# ---------------------------------------------------------------------------
# IAM Role for Enhanced Monitoring
# ---------------------------------------------------------------------------

data "aws_iam_policy_document" "rds_monitoring_assume" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["monitoring.rds.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "rds_monitoring" {
  count = var.db_monitoring_interval > 0 ? 1 : 0

  name               = "${var.name_prefix}-rds-monitoring-role"
  assume_role_policy = data.aws_iam_policy_document.rds_monitoring_assume.json

  tags = {
    Name = "${var.name_prefix}-rds-monitoring-role"
  }
}

resource "aws_iam_role_policy_attachment" "rds_monitoring" {
  count = var.db_monitoring_interval > 0 ? 1 : 0

  role       = aws_iam_role.rds_monitoring[0].name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonRDSEnhancedMonitoringRole"
}

# ---------------------------------------------------------------------------
# RDS Instance
# ---------------------------------------------------------------------------

resource "aws_db_instance" "main" {
  identifier     = "${var.name_prefix}-postgres"
  engine         = "postgres"
  engine_version = var.db_engine_version

  instance_class        = var.db_instance_class
  allocated_storage     = var.db_allocated_storage
  max_allocated_storage = var.db_max_allocated_storage
  storage_type          = "gp3"
  storage_encrypted     = true

  db_name  = var.db_name
  username = var.db_username
  password = random_password.db.result

  db_subnet_group_name   = aws_db_subnet_group.main.name
  vpc_security_group_ids = [var.rds_security_group_id]
  parameter_group_name   = aws_db_parameter_group.main.name

  # High-availability
  multi_az = var.db_multi_az

  # Backups
  backup_retention_period   = var.db_backup_retention_period
  backup_window             = "03:00-04:00"
  maintenance_window        = "Mon:04:00-Mon:05:00"
  copy_tags_to_snapshot     = true
  delete_automated_backups  = false
  final_snapshot_identifier = var.db_skip_final_snapshot ? null : "${var.name_prefix}-final-snapshot"
  skip_final_snapshot       = var.db_skip_final_snapshot
  deletion_protection       = var.db_deletion_protection

  # Performance Insights
  performance_insights_enabled          = var.db_performance_insights_enabled
  performance_insights_retention_period = 7

  # Enhanced Monitoring
  monitoring_interval = var.db_monitoring_interval
  monitoring_role_arn = var.db_monitoring_interval > 0 ? aws_iam_role.rds_monitoring[0].arn : null

  # Apply changes behaviour
  apply_immediately = var.db_apply_immediately

  # Disable public access — only accessible within the VPC
  publicly_accessible = false

  tags = {
    Name = "${var.name_prefix}-postgres"
  }

  lifecycle {
    # Prevent accidental replacement due to password drift
    ignore_changes = [password]
  }
}
