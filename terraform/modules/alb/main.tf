# =============================================================================
# Module: alb — Application Load Balancer (Issue #650)
#
# Creates:
#   - ALB (internet-facing or internal)
#   - S3 bucket for access logs (optional)
#   - HTTP listener (redirects to HTTPS)
#   - HTTPS listener (forwards to target group)
#   - Target group with health checks
# =============================================================================

# ---------------------------------------------------------------------------
# S3 Bucket for ALB Access Logs (optional)
# ---------------------------------------------------------------------------

data "aws_elb_service_account" "main" {}

resource "aws_s3_bucket" "alb_logs" {
  count = var.access_logs_enabled ? 1 : 0

  bucket        = "${var.name_prefix}-alb-access-logs"
  force_destroy = true

  tags = {
    Name    = "${var.name_prefix}-alb-access-logs"
    Purpose = "ALB access logging"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "alb_logs" {
  count  = var.access_logs_enabled ? 1 : 0
  bucket = aws_s3_bucket.alb_logs[0].id

  rule {
    id     = "expire-old-logs"
    status = "Enabled"

    expiration {
      days = 90
    }
  }
}

resource "aws_s3_bucket_policy" "alb_logs" {
  count  = var.access_logs_enabled ? 1 : 0
  bucket = aws_s3_bucket.alb_logs[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect    = "Allow"
        Principal = { AWS = data.aws_elb_service_account.main.arn }
        Action    = "s3:PutObject"
        Resource  = "${aws_s3_bucket.alb_logs[0].arn}/AWSLogs/*"
      }
    ]
  })
}

# ---------------------------------------------------------------------------
# Application Load Balancer
# ---------------------------------------------------------------------------

resource "aws_lb" "main" {
  name               = "${var.name_prefix}-alb"
  internal           = var.internal
  load_balancer_type = "application"
  security_groups    = [var.alb_security_group_id]
  subnets            = var.public_subnet_ids

  idle_timeout               = var.idle_timeout
  enable_deletion_protection = false

  dynamic "access_logs" {
    for_each = var.access_logs_enabled ? [1] : []
    content {
      bucket  = aws_s3_bucket.alb_logs[0].id
      prefix  = "alb"
      enabled = true
    }
  }

  tags = {
    Name = "${var.name_prefix}-alb"
  }
}

# ---------------------------------------------------------------------------
# Target Group
# ---------------------------------------------------------------------------

resource "aws_lb_target_group" "app" {
  name        = "${var.name_prefix}-tg"
  port        = var.app_port
  protocol    = "HTTP"
  vpc_id      = var.vpc_id
  target_type = "ip" # ECS Fargate compatible

  health_check {
    enabled             = true
    path                = var.health_check_path
    interval            = var.health_check_interval
    timeout             = 5
    healthy_threshold   = var.health_check_threshold
    unhealthy_threshold = 3
    matcher             = "200"
  }

  deregistration_delay = 30

  stickiness {
    type    = "lb_cookie"
    enabled = false
  }

  tags = {
    Name = "${var.name_prefix}-tg"
  }

  lifecycle {
    create_before_destroy = true
  }
}

# ---------------------------------------------------------------------------
# HTTP Listener — redirect to HTTPS
# ---------------------------------------------------------------------------

resource "aws_lb_listener" "http" {
  load_balancer_arn = aws_lb.main.arn
  port              = 80
  protocol          = "HTTP"

  default_action {
    type = "redirect"
    redirect {
      port        = "443"
      protocol    = "HTTPS"
      status_code = "HTTP_301"
    }
  }
}

# ---------------------------------------------------------------------------
# HTTPS Listener — forward to target group
# ---------------------------------------------------------------------------

resource "aws_lb_listener" "https" {
  load_balancer_arn = aws_lb.main.arn
  port              = 443
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-TLS13-1-2-2021-06"
  certificate_arn   = var.certificate_arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.app.arn
  }
}
