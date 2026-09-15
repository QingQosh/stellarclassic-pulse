# Multi-Region Deployment for StellarClassicPulse

locals {
  regions = {
    primary   = "us-east-1"
    secondary = "eu-west-1"
    tertiary  = "ap-southeast-1"
  }
}

# Primary Region Deployment
module "stellarclassic_pulse_us_east" {
  source = "./modules/stellarclassic-pulse"
  
  region      = local.regions.primary
  environment = var.environment
  is_primary  = true
  
  cluster_config = {
    instance_type     = "t3.xlarge"
    min_size          = 3
    max_size          = 10
    desired_capacity  = 5
  }
  
  database_config = {
    instance_class    = "db.r6g.2xlarge"
    multi_az          = true
    read_replicas     = 2
  }
}

# Europe Region Deployment
module "stellarclassic_pulse_eu_west" {
  source = "./modules/stellarclassic-pulse"
  
  region      = local.regions.secondary
  environment = var.environment
  is_primary  = false
  
  cluster_config = {
    instance_type     = "t3.large"
    min_size          = 2
    max_size          = 8
    desired_capacity  = 3
  }
  
  database_config = {
    instance_class    = "db.r6g.xlarge"
    multi_az          = true
    read_replicas     = 1
  }
}

# Asia-Pacific Region Deployment
module "stellarclassic_pulse_ap_southeast" {
  source = "./modules/stellarclassic-pulse"
  
  region      = local.regions.tertiary
  environment = var.environment
  is_primary  = false
  
  cluster_config = {
    instance_type     = "t3.large"
    min_size          = 2
    max_size          = 8
    desired_capacity  = 3
  }
  
  database_config = {
    instance_class    = "db.r6g.xlarge"
    multi_az          = true
    read_replicas     = 1
  }
}

# Global Accelerator for multi-region routing
resource "aws_globalaccelerator_accelerator" "stellarclassic_pulse" {
  name            = "stellarclassic-pulse-${var.environment}"
  ip_address_type = "IPV4"
  enabled         = true

  attributes {
    flow_logs_enabled   = true
    flow_logs_s3_bucket = var.flow_logs_bucket
    flow_logs_s3_prefix = "global-accelerator/"
  }
}

resource "aws_globalaccelerator_listener" "stellarclassic_pulse_https" {
  accelerator_arn = aws_globalaccelerator_accelerator.stellarclassic_pulse.id
  protocol        = "TCP"

  port_range {
    from_port = 443
    to_port   = 443
  }
}

# Endpoint groups for each region
resource "aws_globalaccelerator_endpoint_group" "us_east" {
  listener_arn = aws_globalaccelerator_listener.stellarclassic_pulse_https.id
  
  endpoint_group_region = local.regions.primary
  traffic_dial_percentage = 100
  
  health_check_interval_seconds = 30
  health_check_path            = "/health"
  health_check_protocol        = "HTTPS"
  threshold_count              = 3

  endpoint_configuration {
    endpoint_id = module.stellarclassic_pulse_us_east.load_balancer_arn
    weight      = 128
  }
}

resource "aws_globalaccelerator_endpoint_group" "eu_west" {
  listener_arn = aws_globalaccelerator_listener.stellarclassic_pulse_https.id
  
  endpoint_group_region = local.regions.secondary
  traffic_dial_percentage = 80
  
  health_check_interval_seconds = 30
  health_check_path            = "/health"
  health_check_protocol        = "HTTPS"
  threshold_count              = 3

  endpoint_configuration {
    endpoint_id = module.stellarclassic_pulse_eu_west.load_balancer_arn
    weight      = 64
  }
}

resource "aws_globalaccelerator_endpoint_group" "ap_southeast" {
  listener_arn = aws_globalaccelerator_listener.stellarclassic_pulse_https.id
  
  endpoint_group_region = local.regions.tertiary
  traffic_dial_percentage = 80
  
  health_check_interval_seconds = 30
  health_check_path            = "/health"
  health_check_protocol        = "HTTPS"
  threshold_count              = 3

  endpoint_configuration {
    endpoint_id = module.stellarclassic_pulse_ap_southeast.load_balancer_arn
    weight      = 64
  }
}

output "global_accelerator_dns" {
  value       = aws_globalaccelerator_accelerator.stellarclassic_pulse.dns_name
  description = "Global Accelerator DNS name"
}

output "global_accelerator_ips" {
  value       = aws_globalaccelerator_accelerator.stellarclassic_pulse.ip_sets[0].ip_addresses
  description = "Global Accelerator static IPs"
}
