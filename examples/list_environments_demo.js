#!/usr/bin/env node
/**
 * 演示如何使用 list_environments MCP 工具的 Node.js 示例
 */

const https = require('https');
const http = require('http');
const { URL } = require('url');

class MySQLMCPClient {
    constructor(serverUrl = 'http://localhost:8080/mcp') {
        this.serverUrl = serverUrl;
        this.requestId = 0;
    }

    /**
     * 发送 HTTP 请求
     */
    async makeHttpRequest(url, data) {
        return new Promise((resolve, reject) => {
            const urlObj = new URL(url);
            const isHttps = urlObj.protocol === 'https:';
            const client = isHttps ? https : http;

            const options = {
                hostname: urlObj.hostname,
                port: urlObj.port || (isHttps ? 443 : 80),
                path: urlObj.pathname,
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Content-Length': Buffer.byteLength(data)
                }
            };

            const req = client.request(options, (res) => {
                let responseData = '';

                res.on('data', (chunk) => {
                    responseData += chunk;
                });

                res.on('end', () => {
                    try {
                        const result = JSON.parse(responseData);
                        resolve(result);
                    } catch (error) {
                        reject(new Error(`JSON 解析错误: ${error.message}`));
                    }
                });
            });

            req.on('error', (error) => {
                reject(error);
            });

            req.write(data);
            req.end();
        });
    }

    /**
     * 发送 MCP 请求
     */
    async makeRequest(method, params = {}) {
        this.requestId++;

        const payload = {
            jsonrpc: '2.0',
            id: this.requestId,
            method: method,
            params: params
        };

        const result = await this.makeHttpRequest(this.serverUrl, JSON.stringify(payload));

        if (result.error) {
            throw new Error(`MCP Error: ${JSON.stringify(result.error)}`);
        }

        return result.result;
    }

    /**
     * 调用 MCP 工具
     */
    async callTool(toolName, arguments = {}) {
        const params = {
            name: toolName,
            arguments: arguments
        };

        return await this.makeRequest('tools/call', params);
    }

    /**
     * 列出所有环境
     */
    async listEnvironments(includeDisabled = false) {
        return await this.callTool('list_environments', {
            include_disabled: includeDisabled
        });
    }

    /**
     * 获取环境名称列表
     */
    async getEnvironmentNames(includeDisabled = false) {
        const result = await this.listEnvironments(includeDisabled);
        return result.environments.map(env => env.name);
    }

    /**
     * 获取默认环境名称
     */
    async getDefaultEnvironment() {
        const result = await this.listEnvironments();
        return result.default_environment;
    }

    /**
     * 获取特定环境的信息
     */
    async getEnvironmentInfo(envName) {
        const result = await this.listEnvironments(true);
        
        const env = result.environments.find(e => e.name === envName);
        if (!env) {
            throw new Error(`Environment '${envName}' not found`);
        }
        
        return env;
    }
}

async function main() {
    console.log('🌐 MySQL MCP Server - List Environments 演示');
    console.log('='.repeat(50));

    try {
        // 创建客户端
        const client = new MySQLMCPClient();

        // 1. 列出所有启用的环境
        console.log('\n📋 1. 列出所有启用的环境:');
        const environments = await client.listEnvironments();
        console.log(`总共 ${environments.total_count} 个环境`);
        console.log(`默认环境: ${environments.default_environment}`);

        environments.environments.forEach(env => {
            console.log(`  - ${env.name}: ${env.description} (${env.status})`);
        });

        // 2. 列出所有环境（包括禁用的）
        console.log('\n📋 2. 列出所有环境（包括禁用的）:');
        const allEnvironments = await client.listEnvironments(true);

        allEnvironments.environments.forEach(env => {
            const statusIcon = env.status === 'enabled' ? '✅' : '❌';
            const defaultIcon = env.is_default ? '⭐' : '  ';
            console.log(`  ${statusIcon} ${defaultIcon} ${env.name}: ${env.description}`);
        });

        // 3. 获取环境名称列表
        console.log('\n📋 3. 环境名称列表:');
        const envNames = await client.getEnvironmentNames(true);
        console.log(`  ${envNames.join(', ')}`);

        // 4. 获取默认环境
        console.log('\n📋 4. 默认环境:');
        const defaultEnv = await client.getDefaultEnvironment();
        console.log(`  ${defaultEnv}`);

        // 5. 获取特定环境的详细信息
        console.log('\n📋 5. UAT 环境详细信息:');
        try {
            const uatInfo = await client.getEnvironmentInfo('uat');
            console.log(`  名称: ${uatInfo.name}`);
            console.log(`  描述: ${uatInfo.description}`);
            console.log(`  状态: ${uatInfo.status}`);
            console.log(`  主机: ${uatInfo.connection_info.host}`);
            console.log(`  端口: ${uatInfo.connection_info.port}`);
            console.log(`  数据库: ${uatInfo.connection_info.database}`);
            console.log(`  用户名: ${uatInfo.connection_info.username}`);
            console.log(`  最大连接数: ${uatInfo.pool_config.max_connections}`);
        } catch (error) {
            console.log(`  错误: ${error.message}`);
        }

        // 6. 检查环境连接配置
        console.log('\n📋 6. 所有环境连接配置:');
        allEnvironments.environments.forEach(env => {
            const conn = env.connection_info;
            const pool = env.pool_config;
            console.log(`  ${env.name}:`);
            console.log(`    连接: ${conn.username}@${conn.host}:${conn.port}/${conn.database}`);
            console.log(`    连接池: ${pool.min_connections}-${pool.max_connections} 连接`);
        });

        // 7. 以 JSON 格式输出完整信息
        console.log('\n📋 7. 完整环境信息 (JSON):');
        console.log(JSON.stringify(allEnvironments, null, 2));

        console.log('\n✅ 演示完成！');

    } catch (error) {
        if (error.code === 'ECONNREFUSED') {
            console.log('❌ 无法连接到 MySQL MCP Server');
            console.log('请确保服务器正在运行: ./start-real-multi-env.sh');
        } else {
            console.log(`❌ 错误: ${error.message}`);
        }
    }
}

// 运行主函数
if (require.main === module) {
    main().catch(console.error);
}

module.exports = { MySQLMCPClient };