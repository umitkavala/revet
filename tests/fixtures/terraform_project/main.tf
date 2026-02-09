provider "aws" {
  region = "us-east-1"
}

# INFRA Error: Public S3 bucket ACL
# (matches acl\s*=\s*["']public-read)
resource "aws_s3_bucket" "data" {
  bucket = "my-data-bucket"
  acl    = "public-read"
}

# INFRA Error: Open security group 0.0.0.0/0
# (matches cidr_blocks\s*=\s*\[.*["']0\.0\.0\.0/0["'])
resource "aws_security_group" "web" {
  name        = "web-sg"
  description = "Allow all inbound traffic"

  ingress {
    from_port   = 0
    to_port     = 65535
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

# INFRA Info: HTTP backend/source URL
# (matches source\s*=\s*["']http://)
module "vpc" {
  source = "http://example.com/modules/vpc.zip"
}

resource "aws_instance" "app" {
  ami           = "ami-0c55b159cbfafe1f0"
  instance_type = "t3.micro"
  vpc_security_group_ids = [aws_security_group.web.id]
}
