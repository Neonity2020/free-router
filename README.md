# Free Router

Vite + React 前端，Rust / Axum 后端。通过统一的 OpenAI-compatible baseURL 调用 OpenRouter 和 OpenCode Zen 的 Space Bunny。

## 启动

需要 Node.js 20.19+ 和 最新版稳定 Rust。

```sh
npm install --prefix frontend
npm run build --prefix frontend
cargo run --manifest-path backend/Cargo.toml
```

打开 http://127.0.0.1:8787，API baseURL 为 `http://127.0.0.1:8787/v1`。在 Settings 页面填入 OpenRouter 或 OpenCode API Key，点击“保存设置”即可使用，无需重启。密钥保存在根目录 `settings.local.json`，文件已排除 Git，Unix 权限为 0600。前端不回显已保存的密钥，也不写入浏览器存储。未配置密钥时调用返回 503。

开发时分别运行 Rust 后端和 `npm run dev --prefix frontend`，打开 http://127.0.0.1:5173。Vite 将 `/api`、`/v1` 转发到 8787；修改 PORT 后也需要修改 Vite 代理目标。

## 模型路由

| 对外模型 ID | 行为 | 上游模型 |
| --- | --- | --- |
| `space-bunny` | 自动选择已配置上游，默认 OpenCode 优先 | 根据选中的上游映射 |
| `openrouter/space-bunny` | 指定 OpenRouter | `stealth/space-bunny-alpha` |
| `opencode/space-bunny` | 指定 OpenCode Zen | `space-bunny-free` |

也支持上游原始 ID `stealth/space-bunny-alpha` 与 `space-bunny-free`。在 Settings 页面选择优先上游即可改变自动路由顺序。连接失败、HTTP 429 或 5xx 在响应头返回前触发故障切换。已开始输出的流不会重试，避免重复内容。401/403、429、5xx 或连接失败时按轮询顺序尝试池内其他 Key，每个 Key 最多尝试一次；池内耗尽后自动路由可切换上游。400 等其他错误直接返回。上游响应体、工具调用、多模态参数和 SSE 按原协议透传；响应模型名保留上游原始 ID。`x-gateway-provider` 响应头指示实际使用的上游。

## 调用

```sh
curl http://127.0.0.1:8787/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer YOUR_GATEWAY_KEY' \
  -d '{"model":"space-bunny","messages":[{"role":"user","content":"你好"}],"stream":true}'
```

```python
from openai import OpenAI
client = OpenAI(base_url="http://127.0.0.1:8787/v1", api_key="YOUR_GATEWAY_KEY")
response = client.chat.completions.create(
    model="space-bunny",
    messages=[{"role": "user", "content": "你好"}],
)
print(response.choices[0].message.content)
```

在 Settings 的“统一网关 API Key”中生成密钥，复制 Base URL 和 API Key 到其他应用，模型选择 `space-bunny`。已有密钥可加载、显示和复制；重新生成会立即使旧密钥失效。密钥保存在 `gateway-key.local.txt`（Unix 权限 0600），重启后保留，浏览器不持久化。生成后 `/v1` 接口要求 Bearer 密钥，未生成且未配置环境密钥时仍兼容本地免认证调用。

`GATEWAY_API_KEY` 继续作为管理密钥并可调用模型；若设置它，加载或生成密钥、保存设置都需输入该管理密钥。生成的应用密钥不能替代管理密钥。未设置管理密钥时，管理接口保持本地使用方式。密钥读取和生成接口 `/api/gateway-key` 要求 `X-Gateway-Settings: 1`，响应禁止缓存。`GET /v1/models` 同样需要鉴权。`GET /api/status` 只提供配置布尔值和本次进程计数，不返回密钥；“已配置”不代表密钥已通过上游验证。

## 配置

Settings 支持每个上游最多 16 个 API Key，可逐个添加、删除或全部清除，并可调整上游优先级。新输入留空保留旧值，全部清除后即停用该上游。保存会立即生效，已开始的请求仍使用发起时配置。持久化失败时不会修改运行配置。

可选环境配置参见 `.env.example`；Settings 保存值优先于环境配置（包括已清除的密钥）。默认仅监听 `127.0.0.1:8787`，适合本地使用。更改 `HOST` 可以开放监听；对外使用请配置网关密钥。`OPENROUTER_BASE_URL`、`OPENCODE_BASE_URL` 支持覆盖上游地址。请求体上限 10 MiB，连接超时 15 秒，整次请求超时 300 秒。统计随重启清零。

当前实现 Chat Completions 和模型列表，未实现 Responses、Anthropic Messages 协议转换。也可以通过 .env 提供初始凭据。`SETTINGS_FILE` 可指定保存文件路径（父目录须存在）。`POST /api/settings` 需要 `X-Gateway-Settings: 1` 请求头，如果设置了 `GATEWAY_API_KEY`，还需 Bearer 鉴权。

官方接口参考：[OpenRouter Space Bunny](https://openrouter.ai/stealth/space-bunny-alpha)、[OpenCode Zen](https://opencode.ai/docs/en/zen/)。模型可用性与额度由上游控制。

## 验证

```sh
cargo test --manifest-path backend/Cargo.toml
npm run build --prefix frontend
cargo build --manifest-path backend/Cargo.toml
python3 scripts/smoke_test.py
```

Smoke 测试使用本地模拟上游，不需要真实密钥，验证模型映射、自动切换、指定上游、鉴权、SSE、Settings 保存即时生效、重启恢复及清除。

## 应用更新（GitHub Releases）

在 Settings → 应用更新中保存公开仓库 `owner/repo`，默认启用“发现新版本时自动下载”。保存后、网关启动时以及每 6 小时检查最新正式 Release；可以手动检查和下载。取消自动下载后仍定期检查，更新包仅在手动下载时获取。清空仓库并保存可停用检查。当前支持 macOS Apple Silicon、macOS Intel 和 Linux x86_64。

更新配置保存在 `update-settings.local.json`，下载在配置目录的 `.updates/`。仅接受高于当前 Cargo 版本的语义版本正式 Release，按运行平台选择 `free-router-<target>.tar.gz`。需要 GitHub asset 的 `digest` 字段包含 SHA-256。下载流式写入临时文件，验证 SHA-256 与大小后原子重命名；校验失败或中断删除临时文件。包大小上限 150 MiB；下载超时 300 秒。已有包在重启后再次检查时重新校验并复用。并发检查/下载会被拒绝，保存配置时也避免修改正在运行的任务。只有公开仓库支持自动更新，不使用上游 API 密钥访问 GitHub。

下载完成显示“待安装”和本地路径。解压更新包到新目录，停止旧进程，将 `settings.local.json`、`gateway-key.local.txt`、`update-settings.local.json` 和 `.env`（如有）复制到新目录，再运行 `./free-router`。安装前保留旧目录用于回退。网关不会自动替换运行中的程序或重启。

### 发布更新

项目包含 `.github/workflows/release.yml`。将项目放入 GitHub 仓库，更新 `backend/Cargo.toml` 与 `frontend/package.json` 的版本后推送匹配的 `vX.Y.Z` tag。工作流在三个平台构建 Rust 二进制和 React 前端，打包并上传 Release assets。归档包含程序、前端、README 和安装说明，不包含本地配置和 API 密钥。

当前目录尚未关联 GitHub 仓库，需要你填入实际发布仓库后才能检查真实版本。工作流尚未在 GitHub 上运行。

参考：[GitHub Releases API](https://docs.github.com/en/rest/releases/releases)。

## 多 Key 轮询

每个上游拥有独立 Key 池和并发安全的轮询游标，逐请求轮换起始 Key。指定上游模型也支持池内故障切换。流式输出开始后不重试。轮询游标在重启或修改该 Key 池时归零；优先上游调整不会重置未修改的 Key 池。没有冷却或暂停状态，失败 Key 仍参与后续轮询。

旧 `settings.local.json` 中的单字符串 Key 自动兼容；新保存格式为字符串数组。环境变量单 Key 作为初始池。重复 Key 自动去重，状态 API 只返回数量、标签和不可逆标识，不回显 Key。

`POST /api/settings` 保留单字符串替换、`null` 清除，同时支持字符串数组整体替换或 `{"add":["key"],"remove":["key_id"]}` 增量操作。所有 Key 配置仍要求原有管理鉴权和 `X-Gateway-Settings: 1`。保存失败不改变运行池。
