# HTTPS Reverse

一个基于 Cloudflare Pingora 构建的本地 HTTPS 反向代理。项目能够自动生成并信任自签根证书，为 `/etc/hosts`（或 Windows hosts）中指向 `127.0.0.1` 的域名签发通配 TLS 证书，并按 `config.json` 中的路径规则将请求转发到本地各个服务。同时内置 CORS 统一处理以及 WebSocket 透传，方便调试多产品线的前后端。

## 项目特性

- 自动生成根 CA、为本机域名签发 HTTPS 证书，证书存放于 `./tmp`。
- 按最长前缀匹配路径，将请求转发到不同本地端口或上游 URL。
- 统一的 CORS 响应头管理，OPTIONS 预检直接在边缘返回。
- 支持 WebSocket 升级及 HTTP/2，TLS 由 Pingora 提供。

## 快速开始

1. **从 Release 下载制品**  
   前往 [v0.0.13 发布页](https://github.com/aiumz/https_reverse/releases/tag/v0.0.13)，下载对应平台压缩包并解压，得到可执行文件（如 `https_reverse` 或 `https_reverse.exe`）与示例 `config.json`。

2. **生成并信任根证书**

   ```bash
   ./https_reverse trust_root_ca
   ```

   - 证书写入 `./tmp/root_ca.pem`，私钥写入 `./tmp/root_ca_key.pem`。
   - 命令会调用对应平台的工具自动信任该根证书。**首次运行必须执行本步骤；如需新增自定义域名，请在更新 hosts 后重新运行一次以签发新证书。**

3. **编辑 `config.json`**  
   设置监听端口与路由规则（详见下节）。示例文件已覆盖多个 PingCode 产品线，可直接调整端口。

4. **启动 HTTPS 代理**
   ```bash
   ./https_reverse proxy
   ```
   启动后默认监听 `config.json` 中的 `port`（示例为 443），证书位于 `./tmp/local.{crt,key}`。

## 配置说明

`config.json` 示例（完整示例可在 [config.json](https://github.com/aiumz/https_reverse/blob/main/config.json) 查看）：

```json
{
  "port": 443,
  "rules": [
    {
      "name": "agile 前端代理",
      "location": "/static/agile/",
      "proxy_pass": "http://127.0.0.1:11000"
    },
    {
      "name": "agile 服务端代理",
      "location": "/api/agile/",
      "proxy_pass": 11001
    }
  ]
}
```

- `port`：代理监听的 TLS 端口。
- `rules[].name`：可选描述，便于维护。
- `rules[].location`：按最长匹配策略的请求前缀，需以 `/` 结尾以区分不同模块。
- `rules[].proxy_pass`：可写为整型端口（自动补全为 `http://127.0.0.1:<port>`）或完整 URL（支持 `http/https`）。  
  可将前端与后端分别映射到不同端口，通过统一域名解决跨域。

## 运行日志示例

```
==============================================
HTTPS reverse proxy running on port 443 ☺ ☺ ☺
==============================================
[PROXY HTTP] /api/agile/project -> http://127.0.0.1:11001
[PROXY WEBSOCKET] WebSocket upgrade request: /api/pjm/ws
```

## 常见问题

- **证书未被信任**：确认 `cargo run -- trust_root_ca` 以管理员权限执行；Linux 需重新加载 `ca-certificates`。
- **新域名 404 或证书无效**：补充 `hosts` 后需重新执行 `cargo run -- proxy` 以重新签发 `tmp/local.crt`。
- **WebSocket 403**：确保上游服务允许相应路径，并在 `rules` 中指向正确端口。
- **没有执行权限**：下载解压后的二进制需手动授予执行权限，例如 `chmod +x https_reverse` 后再运行。
