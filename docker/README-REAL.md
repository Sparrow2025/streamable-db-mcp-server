# Real Environment Docker Configuration

⚠️ **WARNING: This configuration connects to the actual AWS RDS Aurora database!**

## Overview

This directory contains Docker configuration for connecting to the real AWS RDS Aurora cluster instead of a local MySQL container. This is intended for:

- UAT testing against real data
- Production-like environment testing  
- Integration testing with actual database

## Security Notice

🔒 **IMPORTANT SECURITY INFORMATION:**

- The `docker-compose.real.yml` file contains **REAL DATABASE CREDENTIALS**
- This file is automatically ignored by `.gitignore` 
- **NEVER commit this file to git repository**
- Use only in secure environments
- Rotate credentials regularly

## Files

### Configuration Files
- `docker-compose.real.yml` - Real environment Docker Compose configuration (⚠️ **SENSITIVE**)
- Contains actual AWS RDS Aurora connection details

### Scripts
- `start-real.sh` - Start MCP server with real database connection
- `test-real.sh` - Test real environment functionality  
- `cleanup-real.sh` - Clean up real environment resources

## Quick Start

### 1. Start Real Environment

```bash
# Using script (recommended)
./docker/start-real.sh

# Or using Makefile
make real-start

# Or manually
docker-compose -f docker-compose.real.yml up -d
```

**Note:** The script will ask for confirmation before connecting to the real database.

### 2. Test Real Environment

```bash
# Run comprehensive tests
./docker/test-real.sh

# Or using Makefile
make real-test
```

### 3. View Logs

```bash
# View logs
make real-logs

# Or manually
docker-compose -f docker-compose.real.yml logs -f
```

### 4. Clean Up

```bash
# Stop services
make real-clean

# Or complete cleanup (removes images)
./docker/cleanup-real.sh --full
```

## Database Connection Details

The real environment connects to:

- **Host**: `dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com`
- **Port**: `3306`
- **Database**: `information_schema`
- **User**: `web3-rds`
- **Region**: `ap-southeast-1` (Singapore)

## Available Tests

The test script (`test-real.sh`) performs:

1. ✅ Health check endpoint
2. ✅ MCP protocol initialization  
3. ✅ Tools listing
4. ✅ Database connection test
5. ✅ Query execution against real data
6. ✅ Table listing from information_schema
7. ✅ Container resource monitoring

## Differences from Local Environment

| Aspect | Local Environment | Real Environment |
|--------|------------------|------------------|
| Database | Local MySQL container | AWS RDS Aurora cluster |
| Data | Sample test data | Real UAT data |
| Network | Local Docker network | Internet connection required |
| Security | Development credentials | Production-grade credentials |
| Performance | Local disk I/O | Network latency + Aurora performance |

## Security Best Practices

### 1. Credential Management
- Never hardcode credentials in code
- Use environment variables only
- Rotate credentials regularly
- Use least-privilege database users

### 2. Network Security  
- Ensure secure connection to Aurora
- Use VPN if required by your organization
- Monitor connection logs

### 3. Data Protection
- Be careful with query results containing sensitive data
- Don't log sensitive information
- Follow data privacy regulations

## Troubleshooting

### Connection Issues

```bash
# Check if Aurora cluster is accessible
telnet dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com 3306

# Test credentials manually
mysql -h dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com \
      -P 3306 -u web3-rds -p information_schema
```

### Container Issues

```bash
# Check container status
docker-compose -f docker-compose.real.yml ps

# View detailed logs
docker-compose -f docker-compose.real.yml logs mysql-mcp-server-real

# Check container health
docker inspect mysql-mcp-server-real | grep -A 10 Health
```

### Network Issues

```bash
# Check network connectivity
ping dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com

# Check DNS resolution
nslookup dcs-uat-rds-aurora-cluster.cluster-czcmoige2cq2.ap-southeast-1.rds.amazonaws.com
```

## Monitoring

### Health Endpoints
- **Health Check**: `http://localhost:8080/health`
- **MCP Protocol**: `http://localhost:8080/mcp`

### Metrics
- Container resource usage via `docker stats`
- Application logs via Docker logging
- Database performance via Aurora monitoring

## Production Considerations

### Before Production Use:
1. **Security Review**: Audit all credentials and access patterns
2. **Performance Testing**: Test under expected load
3. **Monitoring Setup**: Configure proper monitoring and alerting
4. **Backup Strategy**: Ensure database backup procedures
5. **Disaster Recovery**: Plan for failure scenarios
6. **Compliance**: Ensure regulatory compliance

### Recommended Production Setup:
- Use AWS Secrets Manager for credentials
- Set up proper VPC and security groups
- Configure SSL/TLS for database connections
- Implement proper logging and monitoring
- Use container orchestration (ECS/EKS)
- Set up load balancing for multiple instances

## Support

For issues with:
- **Docker configuration**: Check this documentation
- **Database connectivity**: Contact database administrator
- **AWS Aurora**: Check AWS console and CloudWatch logs
- **MCP protocol**: Check application logs and health endpoints

---

**Remember**: Always use this configuration responsibly and follow your organization's security policies when working with production data! 🔒