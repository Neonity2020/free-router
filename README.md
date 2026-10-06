# Free Router

Vite + React 前端，Rust / Axum 后端。通过统一的 OpenAI-compatible baseURL 调用 OpenRouter、OpenCode Zen 和 Command Code 上的模型，并在失效时切换到其他上游。

## 启动

需要 Node.js 22.19+ 和 Rust 1.89+（推荐最新版稳定 Rust）。

```sh
npm install --prefix frontend
npm ci --prefix agent
npm run build --prefix frontend
cargo run --manifest-path backend/Cargo.toml
```

打开 http://127.0.0.1:8787，API baseURL 为 `http://127.0.0.1:8787/v1`。在 Settings 页面填入 OpenRouter、OpenCode 或 Command Code API Key，点击“保存设置”即可使用，无需重启。密钥保存在根目录 `settings.local.json`，文件已排除 Git，Unix 权限为 0600。前端不回显已保存的密钥，也不写入浏览器存储。未配置密钥时调用返回 503。

开发时分别运行 Rust 后端和 `npm run dev --prefix frontend`，打开 http://127.0.0.1:5173。Vite 将 `/api`、`/v1` 转发到 8787；修改 PORT 后也需要修改 Vite 代理目标。

## 模型路由

| 对外模型 ID | 行为 | 上游模型 |
| --- | --- | --- |
| `space-bunny` | 自动选择已配置上游，默认 OpenRouter 优先 | 按选中的上游映射，见下方说明 |
| `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` | 指定 OpenRouter | `nvidia/nemotron-3-ultra-550b-a55b:free` |
| `openrouter/apodex/apodex-1.1-mini:free` | 指定 OpenRouter | `apodex/apodex-1.1-mini:free` |
| `opencode/space-bunny` | 指定 OpenCode Zen | `space-bunny-free` |
| `commandcode/space-bunny` | 指定 Command Code | `stealth/space-bunny-alpha` |
| `commandcode/<模型ID>` | 指定 Command Code 的其他 Chat Completions 模型 | 去掉 `commandcode/` 前缀，保留原模型 ID |

Command Code 密钥在 Settings 中填写，支持最多 16 个 Key 的轮询、添加和删除，也可设为自动路由优先上游。默认 Provider API baseURL 为 `https://api.commandcode.ai/provider/v1`，可用 `COMMANDCODE_BASE_URL` 覆盖；环境变量密钥为 `COMMANDCODE_API_KEY`。未配置 Command Code 时不参与自动路由。除 Space Bunny 别名外，例如 `commandcode/deepseek/deepseek-v4-flash` 会原样转发 `deepseek/deepseek-v4-flash`，可用模型以官方目录为准。本网关当前提供 Chat Completions，不将 Claude 的 Anthropic Messages 或 Responses 协议转换为聊天接口。参考：[Command Code Provider API](https://commandcode.ai/docs/provider)、[Studio API Key](https://commandcode.ai/studio/)。

`space-bunny` 别名在各上游的映射：OpenRouter → `nvidia/nemotron-3-ultra-550b-a55b:free`（默认优先），OpenCode Zen → `space-bunny-free`，Command Code → `stealth/space-bunny-alpha`。原来的 `stealth/space-bunny-alpha` 已从 OpenRouter 下架，因此 OpenRouter 不再提供 Space Bunny；`openrouter/<模型ID>` 会原样转发上游模型 ID。也支持上游原始 ID `space-bunny-free`。在 Settings 页面选择优先上游即可改变自动路由顺序（默认 `openrouter`）。连接失败、HTTP 429 或 5xx 在响应头返回前触发故障切换。已开始输出的流不会重试，避免重复内容。401/403、429、5xx 或连接失败时按轮询顺序尝试池内其他 Key，每个 Key 最多尝试一次；池内耗尽后自动路由可切换上游。上游把失败包进 200 响应体时（OpenRouter 的 `provider_overloaded` 就是这种形态，JSON 与 SSE 两种），网关会在响应头发出前识别并按同样规则切换：非流式先读完 JSON 响应体，流式先预读首个 SSE 事件（因此流式响应头会等到首个事件到达），确实没有可选上游时才原样透传。400 等其他错误直接返回。上游响应体、工具调用、多模态参数和 SSE 按原协议透传；响应模型名保留上游原始 ID。`x-gateway-provider` 响应头指示实际使用的上游。

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

可选环境配置参见 `.env.example`；Settings 保存值优先于环境配置（包括已清除的密钥）。默认仅监听 `127.0.0.1:8787`，适合本地使用。更改 `HOST` 可以开放监听；对外使用请配置网关密钥。`OPENROUTER_BASE_URL`、`OPENCODE_BASE_URL` 支持覆盖上游地址。请求体上限 10 MiB，连接超时 15 秒，模型调用总超时默认 300 秒（`GATEWAY_REQUEST_TIMEOUT_SECS` 可设为 1–86400 秒，修改后重启）。所有 Key 重试和上游切换共享同一时间预算，响应体与 SSE 也计入预算；预算耗尽后不再重试，响应头尚未发送时返回 504，已开始的响应则终止传输。统计随重启清零。

当前实现 Chat Completions 和模型列表，未实现 Responses、Anthropic Messages 协议转换。也可以通过 .env 提供初始凭据。`SETTINGS_FILE` 可指定保存文件路径（父目录会自动创建）。`POST /api/settings` 需要 `X-Gateway-Settings: 1` 请求头，如果设置了 `GATEWAY_API_KEY`，还需 Bearer 鉴权。

官方接口参考：[OpenRouter Nemotron 3 Ultra](https://openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b:free)、[OpenCode Zen](https://opencode.ai/docs/en/zen/)。模型可用性与额度由上游控制。

## 命令行

同一个 `free-router` 二进制无参数时启动网关；传入子命令则在终端管理本地网关，适合无桌面或远程 SSH 环境。命令默认连接 `HOST`/`PORT`（默认 `http://127.0.0.1:8787`）上的运行实例，可写操作优先走管理接口、立即生效；只有目标网关无法连接时才进入离线模式，直接读写本地配置，启动后生效并在 stderr 提示。管理鉴权失败、请求超时或目标端口为其他服务时直接报错，不修改本地配置。网关在运行期间持有配置目录的进程锁；离线写入最多等待锁 5 秒，防止并发 CLI 丢失修改或覆盖运行中的配置。进程退出（包括异常终止）后锁自动释放。

```sh
free-router status                 # 运行状态、默认上游、各上游 Key 数量
free-router status --json
free-router keys list              # 列出各上游 Key 的不可逆 ID 与标签
free-router keys add opencode sk-a sk-b    # 追加（重复自动忽略，每家上限 16 个）
free-router keys remove opencode key_ab12cd    # 完整 ID 或唯一前缀
free-router keys clear commandcode         # 清空某上游
free-router provider               # 查看默认上游
free-router provider openrouter    # 切换默认上游
free-router gateway-key show       # 显示 /v1 使用的网关密钥
free-router gateway-key generate   # 生成并保存新密钥（旧密钥立即失效）
free-router config                 # 显示解析后的配置路径与网关地址
free-router serve                  # 显式启动网关（等同无参数）
```

`status`、`keys list` 支持 `--json`，便于脚本消费。只显示不可逆 Key ID，不回显密钥；留空或未知上游会报错并列出可选值。离线读取与修改会继承尚未被本地配置覆盖的环境变量 Key；显式清空的 Key 池不会被环境变量恢复。写入使用独占随机临时文件、原子替换与 Unix 0600 权限。`gateway-key show` 在线时读取管理接口中的当前密钥，离线时才读取本地文件。

想在任意目录直接调用，可注册为系统命令：`npm run cli:install` 会构建前端、安装 Agent 依赖并构建 release 二进制，将命令复制到 `~/.local/bin`，将前端与 Agent 资源复制到同级 `free-router-resources/`（可用 `FREE_ROUTER_BIN_DIR` 覆盖目标目录，非 Windows 平台命令权限为 755）。安装不复制配置或密钥；资源复制失败时保留旧安装。安装完成后可删除或移动源码目录，网关、Web UI 与 Agent 均从安装资源运行，Agent 仍需要 Node.js 22.19+。改完代码需重新安装。

安装版默认配置目录为 macOS 的 `~/Library/Application Support/Free Router`、Linux 的 `$XDG_CONFIG_HOME/free-router`（默认 `~/.config/free-router`）、Windows 的 `%APPDATA%/Free Router`。开发版继续使用项目根目录，便携发布包继续使用包所在目录；`SETTINGS_FILE` 优先覆盖配置路径，`FREE_ROUTER_RESOURCE_ROOT` 可覆盖资源目录（未设 `SETTINGS_FILE` 时也作为配置默认目录）。可通过 `free-router config` 查看实际路径。安装前已有项目配置不会自动迁移，如需继续使用，可显式指定 `SETTINGS_FILE`，或在停止网关后将配置文件复制到安装版配置目录。

`.env` 从配置目录和资源目录加载（配置目录优先），不会因切换工作目录而加载其他项目的 `.env`。`HOST`、`PORT`、`GATEWAY_API_KEY` 与显式环境变量仍可覆盖 `.env`。

## 验证

前端与 Agent 的集成测试会启动真实的 Rust 网关二进制，**必须先构建**，否则会以 Node 原生崩溃堆栈而非断言失败报错。

```sh
cargo test --manifest-path backend/Cargo.toml
npm run build --prefix frontend
cargo build --manifest-path backend/Cargo.toml

npm test --prefix frontend
npm test --prefix agent
npm run cli:test
npm run desktop:test

python3 scripts/smoke_test.py
python3 scripts/retry_timeout_test.py
python3 scripts/cooldown_test.py
python3 scripts/cli_smoke_test.py
```

`npm test --prefix frontend` 与 `npm test --prefix agent` 依赖 `backend/target/debug/free-router`；缺失时会直接提示需要先执行 `cargo build`。

Smoke 测试使用本地模拟上游，不需要真实密钥，验证模型映射、自动切换、指定上游、鉴权、SSE、Settings 保存即时生效、重启恢复及清除。

## 应用更新（GitHub Releases）

在 Settings → 应用更新中保存公开仓库 `owner/repo`，默认启用“发现新版本时自动下载”。保存后、网关启动时以及每 6 小时检查最新正式 Release；可以手动检查和下载。取消自动下载后仍定期检查，更新包仅在手动下载时获取。清空仓库并保存可停用检查。当前支持 macOS Apple Silicon、macOS Intel 和 Linux x86_64。

更新配置保存在 `update-settings.local.json`，下载在配置目录的 `.updates/`。仅接受高于当前 Cargo 版本的语义版本正式 Release，按运行平台选择 `free-router-<target>.tar.gz`。需要 GitHub asset 的 `digest` 字段包含 SHA-256。下载流式写入临时文件，验证 SHA-256 与大小后原子重命名；校验失败或中断删除临时文件。包大小上限 150 MiB；下载超时 300 秒。已有包在重启后再次检查时重新校验并复用。并发检查/下载会被拒绝，保存配置时也避免修改正在运行的任务。只有公开仓库支持自动更新，不使用上游 API 密钥访问 GitHub。

下载完成显示“待安装”和本地路径。解压更新包到新目录，停止旧进程，将 `settings.local.json`、`gateway-key.local.txt`、`update-settings.local.json` 和 `.env`（如有）复制到新目录，再运行 `./free-router`。安装前保留旧目录用于回退。网关不会自动替换运行中的程序或重启。

### 发布更新

项目包含 `.github/workflows/release.yml`。将项目放入 GitHub 仓库，更新 `backend/Cargo.toml` 与 `frontend/package.json` 的版本后推送匹配的 `vX.Y.Z` tag。工作流在三个平台构建 Rust 二进制和 React 前端，打包并上传 Release assets。归档包含程序、前端、README 和安装说明，不包含本地配置和 API 密钥。

仓库已关联 `Neonity2020/free-router`（公开）。工作流尚未在 GitHub 上运行：当前没有任何 `vX.Y.Z` tag 或 Release，因此应用内更新检查还没有可发现的真实版本。

参考：[GitHub Releases API](https://docs.github.com/en/rest/releases/releases)。

## 多 Key 轮询

每个上游拥有独立 Key 池和并发安全的轮询游标，逐请求轮换起始 Key。指定上游模型也支持池内故障切换。流式输出开始后不重试。轮询游标在重启或修改该 Key 池时归零；优先上游调整不会重置未修改的 Key 池。没有冷却或暂停状态，失败 Key 仍参与后续轮询。

旧 `settings.local.json` 中的单字符串 Key 自动兼容；新保存格式为字符串数组。环境变量单 Key 作为初始池。重复 Key 自动去重，状态 API 只返回数量、标签和不可逆标识，不回显 Key。

`POST /api/settings` 保留单字符串替换、`null` 清除，同时支持字符串数组整体替换或 `{"add":["key"],"remove":["key_id"]}` 增量操作。所有 Key 配置仍要求原有管理鉴权和 `X-Gateway-Settings: 1`。保存失败不改变运行池。

## 外观主题

顶部“外观主题”可选择跟随系统、浅色或暗色。默认跟随系统并响应系统外观变化，手动选择保存在浏览器本地；刷新后恢复，多个同源页面同步选择。主题不影响网关配置或 API Key。

## Pi AI SDK

已集成官方 `@earendil-works/pi-ai@0.99.2`，使用当前 `createModels` / `createProvider` API。Playground 通过 Pi SDK 调用本地网关，支持流式回复、停止生成、思考内容与上游返回的 token 用量。上游未返回用量时不显示统计。上游 API Key 仍只由 Rust 后端读取；SDK 仅接收本地网关密钥，不持久化到浏览器。

“开始调用”中选择 **Pi AI SDK**，可复制完整的 Node.js 示例，支持四个网关模型 ID。安装命令：`npm install @earendil-works/pi-ai@0.99.2`，通过 `GATEWAY_API_KEY` 环境变量传入 Settings 中生成的网关密钥。示例的上下文窗口 32768 和输出上限 4096 是保守的本地默认值，不代表上游真实限制；示例费用元数据为占位值，应用不显示费用估算。

SDK 要求 Node.js 22.19+。仅按需加载 OpenAI Chat Completions 适配器。Rust 后端继续提供统一路由、Key 轮询和故障切换。

验证（使用隔离配置和模拟上游，无需真实密钥）：

```sh
cargo build --manifest-path backend/Cargo.toml
npm test --prefix frontend
npm run build --prefix frontend
```

参考：[Pi AI 官方文档](https://github.com/earendil-works/pi/tree/main/packages/ai)。

## Electron 桌面应用

桌面应用内置 Rust 网关、前端和 Pi Agent 依赖；打包版本使用 Electron 自带的 Node 运行 Agent，不需要用户单独安装 Node。窗口提供原生文件夹选择。启动默认使用 `http://127.0.0.1:8787/v1`，端口被占用时自动选择后续空闲端口，以应用显示的 baseURL 为准。应用内管理请求自动授权，外部模型客户端使用 Settings 中生成的网关密钥。

```sh
npm ci
npm ci --prefix frontend
npm ci --prefix agent
npm run desktop:dev   # 构建前端与 Rust 后启动 Electron
npm run desktop:pack  # 构建当前平台的桌面应用目录
npm run desktop:dist  # macOS DMG/ZIP；Linux AppImage；Windows NSIS
```

需要构建平台对应的 Rust 工具链。桌面应用配置保存在 Electron `userData`（macOS 默认 `~/Library/Application Support/Free Router`），与应用安装目录分离。开发版首次启动会导入项目已保存的配置，不覆盖已有桌面配置；发布版不包含任何本地密钥。macOS 关闭窗口仍保留网关，菜单“退出 Free Router”停止网关与 Agent。桌面安装包目前为未签名构建，未接入桌面自动升级；Settings 提供 Releases 下载入口。原有 Web/CLI 发布与更新流程保持可用。

## Pi Agent Web UI

在 Settings 中填写 **Exa API Key** 并点击“保存设置”，Pi Agent 即可调用 `web_search` 搜索网页，返回标题、来源链接和内容摘要。密钥保存在现有本地设置文件，不回显到状态接口或会话记录；留空保留原密钥，可勾选清除。已创建会话在下次任务读取最新密钥，无需重建会话。工具支持取消和 30 秒超时，默认 5 条结果（可指定 1–10 条）。参考：[Exa Search API](https://exa.ai/docs/reference/search)。

macOS 上可点击“Finder 选择”调用系统文件夹选择窗口；选定后自动填写规范化的绝对路径，取消则保留原目录。窗口默认打开当前填写的有效目录，路径也可手动编辑。选择窗口只允许本机访问，其他系统或远程访问可直接手动输入目录。

连续工具调用汇总为默认收起的活动记录，显示工具名称、调用次数及运行/失败状态，展开后可查看每次调用的参数与完整日志。相邻且参数、输出完全相同的成功调用合并展示并标注次数；失败与正在运行的调用单独保留。空助手工具轮次不再重复占位，轮询不会重置展开状态。同一调用 ID 的重复事件更新原记录，迟到的部分输出不会覆盖最终结果；此展示合并不改变工具的实际执行。

Pi Agent 和 Playground 的模型回复及思考内容支持 Markdown：标题、加粗、引用、列表、表格、任务清单、链接和代码块。代码块显示语言标识并可一键复制，长代码及表格可以横向滚动；浅色与暗色主题均有对应样式。流式生成期间实时更新，工具参数和终端日志保持纯文本。原始 HTML 不执行，链接保留默认安全 URL 过滤。

左侧 **Pi Agent** 使用官方 `@earendil-works/pi-coding-agent@0.99.2` 提供编程代理。选择一个已存在的绝对工作目录及网关模型，点击“新建编程会话”，即可通过连续对话阅读代码、创建文件、精确编辑、运行命令和测试。界面实时更新回复、工具参数、执行输出和失败状态；“停止任务”会取消模型调用及正在运行的工具。可切换或删除会话，删除仅清除对话记录，不撤销文件修改。

Agent 直接使用现有网关配置、上游 Key 轮询和自动故障切换，无需另填上游密钥。若设置 `GATEWAY_API_KEY`，进入 Agent 后须输入管理密钥；普通模型调用密钥不能授权编程接口。Rust 在首次打开 Agent 时启动 Node.js 服务，并通过随机内部令牌连接到仅监听 loopback 的随机端口。Rust 退出时停止 Agent 与工具。更换网关密钥后，下一次任务使用当前凭据。

从其他机器访问 Pi Agent 必须设置 `GATEWAY_API_KEY` 管理密钥；未设置时 Agent 接口只接受 loopback 连接。浏览器请求还要求管理自定义头，跨域站点不能通过表单触发编程操作。

工具以本机用户权限执行，工作目录用于路径解析，**不是安全沙箱**；仅在信任的本地环境使用。默认工作目录为项目根目录，可通过 `PI_AGENT_WORKSPACE` 指定；`PI_NODE_BIN` 可指定 Node 可执行文件。Agent 不加载全局 Pi 扩展、登录凭据或其他模型配置。会话保存在内存，刷新 Web UI 可以继续，服务重启后会话清空，文件修改保留。最多保留 12 个会话，同时运行一个任务；界面最多展示最近 160 条记录，长工具输出会截断。实际模型的工具调用支持和可用性仍由上游决定。

开发环境：`npm ci --prefix agent`，然后启动 Rust 网关。发布包包含 Agent 依赖，但本机仍需要 Node.js 22.19+；只使用网关代理时不需要启动 Node Agent。

端到端验证：

```sh
cargo build --manifest-path backend/Cargo.toml
npm test --prefix agent
```

测试用模拟模型驱动真实 SDK，在临时目录完成 `write → edit → bash → read`，验证对话、鉴权、并发拒绝、停止工具和删除会话。参考：[Pi Coding Agent SDK](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/sdk.md)。

依赖审计：官方 SDK 0.99.2 的 shrinkwrap 固定了存在拒绝服务漏洞的 `brace-expansion@5.0.9`。Agent 直接锁定修复版本 `5.0.12`，安装脚本用 npm 校验完整性的包替换 SDK 内部副本；安全测试检查实际解析的版本，避免仅修改锁文件造成审计与运行版本不一致。安装 Agent 时请使用正常的 `npm ci --prefix agent`，确保执行安装脚本。参考：[上游漏洞公告](https://github.com/advisories/GHSA-q2hr-2g5m-vwhr)。

推理参数：OpenCode Zen 的 Space Bunny 路由支持 `low`、`medium`、`high`、`xhigh`、`max` 五档推理，`minimal` 实测可接受，但不能据此认定为独立档位。OpenRouter 默认模型 `nvidia/nemotron-3-ultra-550b-a55b:free` 的上游声明是推理可选、只提供 `high` 与 `medium` 两档、默认 `high`，`/v1/models` 按上游声明上报。只有 OpenCode Zen 的 Space Bunny 路由不支持关闭推理：`reasoning_effort: "none"`、`reasoning.effort: "none"`、`reasoning.enabled: false`、`thinking.type: "disabled"` 或 `enable_thinking: false` 会返回明确的 400；其他路由保留原始参数，由上游决定是否接受，自动路由也会尝试下一个上游。Command Code 的等级能力尚未通过真实账号验证，不宣称支持特定档位。Pi Agent 默认开启 `high`，Playground 使用上游默认等级。Settings 支持粘贴带 `Bearer` 前缀或首尾空白的 API Key，格式错误提示上游与密钥序号，不回显密钥。

失败 Key 冷却：401/403 默认暂停 300 秒；429 和 5xx 默认暂停 30 秒，有整数秒 `Retry-After` 时优先使用；连接失败暂停 5 秒。请求会跳过冷却中的 Key，继续尝试其他 Key 或自动路由的其他上游。全部 Key 冷却时返回 503 和最早可重试的 `Retry-After`，不会占用请求预算等待。保留同一 Key 的设置更新不会清除冷却状态；重启清空内存状态。`GATEWAY_KEY_COOLDOWN_SECS` 可统一覆盖为 0–86400 秒，0 关闭冷却，适合独立的重试测试。

诊断：模型请求返回 `x-gateway-request-id`，stderr 输出 JSON 格式的上游尝试和请求完成记录，包含请求编号、上游、尝试序号、状态、耗时及流完成/中断/取消结果。记录不包含凭证、地址或请求/响应正文。`GATEWAY_LOG_REQUESTS=0` 可关闭记录。

PR 和 main 分支推送会运行 `.github/workflows/ci.yml`：Rust 格式、Clippy、单元测试，前端构建，Agent 安全审计及运行版本检查，以及前端、CLI、桌面和本地网关集成测试。网关职责拆分到 `gateway`、`keys`、`reasoning`、`settings`、`diagnostics`；前端拆分概览、设置面板及设置状态，减少入口文件的维护负担。
