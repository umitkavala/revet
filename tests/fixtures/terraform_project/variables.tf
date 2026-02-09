variable "region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Deployment environment"
  type        = string
  default     = "production"
}

# INFRA Error: Hardcoded provider credentials
# (matches access_key\s*=\s*["'][A-Za-z0-9/+=]{16,}["'])
variable "aws_config" {
  description = "AWS configuration"
  type        = map(string)
  default = {
    access_key = "AKIAIOSFODNN7EXAMPLEKEY"
  }
}
