# =============================================================================
# Module: monitoring — CloudWatch Alarms & Log Groups (Issue #650)
#
# Creates:
#   - CloudWatch Log Group for the application
#   - SNS Topic for alarm notifications
#   - RDS alarms: CPU, free storage, DB connections, replica lag
#   - ALB alarms: 5xx error rate, p99 latency, unhealthy host count
# =============================================================================

# ---------------------------------------------------------------------------
# CloudWatch Log Group
# ---------------------------------------------------------------------------

resource "aws_cloudwatch_log_group" "app" {
  name              = "/soroban-pulse/${var.name_prefix}"
  retention_in_days = var.log_retention_days

  tags = {
    Name = "${var.name_prefix}-app-logs"
  }
}

# ---------------------------------------------------------------------------
# SNS Topic for Alarms
# ---------------------------------------------------------------------------

resource "aws_sns_topic" "alarms" {
  name = "${var.name_prefix}-alarms"

  tags = {
    Name = "${var.name_prefix}-alarms"
  }
}

# Subscribe any externally-provided ARNs (e.g., PagerDuty HTTPS endpoint)
resource "aws_sns_topic_subscription" "alarm_subscriptions" {
  count = length(var.alarm_actions_arn)

  topic_arn = aws_sns_topic.alarms.arn
  protocol  = "https"
  endpoint  = var.alarm_actions_arn[count.index]
}

# ---------------------------------------------------------------------------
# RDS Alarms
# ---------------------------------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "rds_cpu" {
  alarm_name          = "${var.name_prefix}-rds-cpu-high"
  alarm_description   = "RDS CPU utilisation exceeded ${var.rds_cpu_alarm_threshold}%."
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "CPUUtilization"
  namespace           = "AWS/RDS"
  period              = 300
  statistic           = "Average"
  threshold           = var.rds_cpu_alarm_threshold
  treat_missing_data  = "notBreaching"

  dimensions = {
    DBInstanceIdentifier = var.rds_instance_id
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-rds-cpu-high"
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_storage" {
  alarm_name          = "${var.name_prefix}-rds-storage-low"
  alarm_description   = "RDS free storage is below ${var.rds_storage_alarm_threshold / 1073741824} GiB."
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  metric_name         = "FreeStorageSpace"
  namespace           = "AWS/RDS"
  period              = 300
  statistic           = "Average"
  threshold           = var.rds_storage_alarm_threshold
  treat_missing_data  = "notBreaching"

  dimensions = {
    DBInstanceIdentifier = var.rds_instance_id
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-rds-storage-low"
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_connections" {
  alarm_name          = "${var.name_prefix}-rds-connections-high"
  alarm_description   = "RDS database connection count is high (>= 80)."
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 3
  metric_name         = "DatabaseConnections"
  namespace           = "AWS/RDS"
  period              = 60
  statistic           = "Maximum"
  threshold           = 80
  treat_missing_data  = "notBreaching"

  dimensions = {
    DBInstanceIdentifier = var.rds_instance_id
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-rds-connections-high"
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_read_latency" {
  alarm_name          = "${var.name_prefix}-rds-read-latency"
  alarm_description   = "RDS read latency exceeded 20ms."
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "ReadLatency"
  namespace           = "AWS/RDS"
  period              = 60
  statistic           = "p99"
  threshold           = 0.02 # 20ms
  treat_missing_data  = "notBreaching"

  dimensions = {
    DBInstanceIdentifier = var.rds_instance_id
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-rds-read-latency"
  }
}

# ---------------------------------------------------------------------------
# ALB Alarms
# ---------------------------------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "alb_5xx" {
  alarm_name          = "${var.name_prefix}-alb-5xx-high"
  alarm_description   = "ALB 5xx error count exceeded ${var.alb_5xx_alarm_threshold} per minute."
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "HTTPCode_Target_5XX_Count"
  namespace           = "AWS/ApplicationELB"
  period              = 60
  statistic           = "Sum"
  threshold           = var.alb_5xx_alarm_threshold
  treat_missing_data  = "notBreaching"

  dimensions = {
    LoadBalancer = var.alb_arn_suffix
    TargetGroup  = var.target_group_arn_suffix
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-alb-5xx-high"
  }
}

resource "aws_cloudwatch_metric_alarm" "alb_latency" {
  alarm_name          = "${var.name_prefix}-alb-p99-latency"
  alarm_description   = "ALB target response p99 latency exceeded ${var.alb_latency_alarm_threshold}s."
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  extended_statistic  = "p99"
  metric_name         = "TargetResponseTime"
  namespace           = "AWS/ApplicationELB"
  period              = 60
  threshold           = var.alb_latency_alarm_threshold
  treat_missing_data  = "notBreaching"

  dimensions = {
    LoadBalancer = var.alb_arn_suffix
    TargetGroup  = var.target_group_arn_suffix
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-alb-p99-latency"
  }
}

resource "aws_cloudwatch_metric_alarm" "alb_unhealthy_hosts" {
  alarm_name          = "${var.name_prefix}-alb-unhealthy-hosts"
  alarm_description   = "One or more ALB target group hosts are unhealthy."
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 2
  metric_name         = "UnHealthyHostCount"
  namespace           = "AWS/ApplicationELB"
  period              = 60
  statistic           = "Maximum"
  threshold           = 1
  treat_missing_data  = "notBreaching"

  dimensions = {
    LoadBalancer = var.alb_arn_suffix
    TargetGroup  = var.target_group_arn_suffix
  }

  alarm_actions = concat([aws_sns_topic.alarms.arn], var.alarm_actions_arn)
  ok_actions    = concat([aws_sns_topic.alarms.arn], var.ok_actions_arn)

  tags = {
    Name = "${var.name_prefix}-alb-unhealthy-hosts"
  }
}

# ---------------------------------------------------------------------------
# CloudWatch Dashboard
# ---------------------------------------------------------------------------

resource "aws_cloudwatch_dashboard" "main" {
  dashboard_name = "${var.name_prefix}-overview"

  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "RDS CPU Utilization"
          period = 300
          stat   = "Average"
          metrics = [
            ["AWS/RDS", "CPUUtilization", "DBInstanceIdentifier", var.rds_instance_id]
          ]
          view    = "timeSeries"
          stacked = false
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "RDS Free Storage Space"
          period = 300
          stat   = "Average"
          metrics = [
            ["AWS/RDS", "FreeStorageSpace", "DBInstanceIdentifier", var.rds_instance_id]
          ]
          view    = "timeSeries"
          stacked = false
        }
      },
      {
        type   = "metric"
        x      = 0
        y      = 6
        width  = 12
        height = 6
        properties = {
          title  = "ALB 5xx Errors"
          period = 60
          stat   = "Sum"
          metrics = [
            ["AWS/ApplicationELB", "HTTPCode_Target_5XX_Count",
              "LoadBalancer", var.alb_arn_suffix,
              "TargetGroup", var.target_group_arn_suffix]
          ]
          view    = "timeSeries"
          stacked = false
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 6
        width  = 12
        height = 6
        properties = {
          title      = "ALB p99 Response Time"
          period     = 60
          stat       = "p99"
          metrics = [
            ["AWS/ApplicationELB", "TargetResponseTime",
              "LoadBalancer", var.alb_arn_suffix,
              "TargetGroup", var.target_group_arn_suffix]
          ]
          view    = "timeSeries"
          stacked = false
        }
      }
    ]
  })
}
