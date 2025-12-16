#!/usr/bin/env python3
"""
演示如何使用 list_environments MCP 工具的 Python 示例
"""

import json
import requests
from typing import Dict, List, Any

class MySQLMCPClient:
    """MySQL MCP Server 客户端"""
    
    def __init__(self, server_url: str = "http://localhost:8080/mcp"):
        self.server_url = server_url
        self.request_id = 0
    
    def _make_request(self, method: str, params: Dict[str, Any] = None) -> Dict[str, Any]:
        """发送 MCP 请求"""
        self.request_id += 1
        
        payload = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params or {}
        }
        
        response = requests.post(
            self.server_url,
            headers={"Content-Type": "application/json"},
            json=payload
        )
        response.raise_for_status()
        return response.json()
    
    def call_tool(self, tool_name: str, arguments: Dict[str, Any] = None) -> Any:
        """调用 MCP 工具"""
        params = {
            "name": tool_name,
            "arguments": arguments or {}
        }
        
        result = self._make_request("tools/call", params)
        
        if "error" in result:
            raise Exception(f"MCP Error: {result['error']}")
        
        return result.get("result")
    
    def list_environments(self, include_disabled: bool = False) -> Dict[str, Any]:
        """列出所有环境"""
        return self.call_tool("list_environments", {
            "include_disabled": include_disabled
        })
    
    def get_environment_names(self, include_disabled: bool = False) -> List[str]:
        """获取环境名称列表"""
        result = self.list_environments(include_disabled)
        return [env["name"] for env in result["environments"]]
    
    def get_default_environment(self) -> str:
        """获取默认环境名称"""
        result = self.list_environments()
        return result["default_environment"]
    
    def get_environment_info(self, env_name: str) -> Dict[str, Any]:
        """获取特定环境的信息"""
        result = self.list_environments(include_disabled=True)
        
        for env in result["environments"]:
            if env["name"] == env_name:
                return env
        
        raise ValueError(f"Environment '{env_name}' not found")

def main():
    """主函数"""
    print("🌐 MySQL MCP Server - List Environments 演示")
    print("=" * 50)
    
    try:
        # 创建客户端
        client = MySQLMCPClient()
        
        # 1. 列出所有启用的环境
        print("\n📋 1. 列出所有启用的环境:")
        environments = client.list_environments()
        print(f"总共 {environments['total_count']} 个环境")
        print(f"默认环境: {environments['default_environment']}")
        
        for env in environments["environments"]:
            print(f"  - {env['name']}: {env['description']} ({env['status']})")
        
        # 2. 列出所有环境（包括禁用的）
        print("\n📋 2. 列出所有环境（包括禁用的）:")
        all_environments = client.list_environments(include_disabled=True)
        
        for env in all_environments["environments"]:
            status_icon = "✅" if env["status"] == "enabled" else "❌"
            default_icon = "⭐" if env["is_default"] else "  "
            print(f"  {status_icon} {default_icon} {env['name']}: {env['description']}")
        
        # 3. 获取环境名称列表
        print("\n📋 3. 环境名称列表:")
        env_names = client.get_environment_names(include_disabled=True)
        print(f"  {', '.join(env_names)}")
        
        # 4. 获取默认环境
        print("\n📋 4. 默认环境:")
        default_env = client.get_default_environment()
        print(f"  {default_env}")
        
        # 5. 获取特定环境的详细信息
        print("\n📋 5. UAT 环境详细信息:")
        try:
            uat_info = client.get_environment_info("uat")
            print(f"  名称: {uat_info['name']}")
            print(f"  描述: {uat_info['description']}")
            print(f"  状态: {uat_info['status']}")
            print(f"  主机: {uat_info['connection_info']['host']}")
            print(f"  端口: {uat_info['connection_info']['port']}")
            print(f"  数据库: {uat_info['connection_info']['database']}")
            print(f"  用户名: {uat_info['connection_info']['username']}")
            print(f"  最大连接数: {uat_info['pool_config']['max_connections']}")
        except ValueError as e:
            print(f"  错误: {e}")
        
        # 6. 检查环境连接配置
        print("\n📋 6. 所有环境连接配置:")
        for env in all_environments["environments"]:
            conn = env["connection_info"]
            pool = env["pool_config"]
            print(f"  {env['name']}:")
            print(f"    连接: {conn['username']}@{conn['host']}:{conn['port']}/{conn['database']}")
            print(f"    连接池: {pool['min_connections']}-{pool['max_connections']} 连接")
        
        print("\n✅ 演示完成！")
        
    except requests.exceptions.ConnectionError:
        print("❌ 无法连接到 MySQL MCP Server")
        print("请确保服务器正在运行: ./start-real-multi-env.sh")
    except Exception as e:
        print(f"❌ 错误: {e}")

if __name__ == "__main__":
    main()