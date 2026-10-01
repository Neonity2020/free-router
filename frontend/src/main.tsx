import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  ArrowUpRight,
  Check,
  Copy,
  Network,
  Play,
  Radio,
  Settings2,
  Terminal,
  Zap,
} from "lucide-react";
import "./style.css";
import UpdateSettings from "./UpdateSettings";
import CopyModelId from "./CopyModelId";
type Status = {
  auth_required: boolean;
  management_auth_required: boolean;
  default_provider: string;
  requests: number;
  fallbacks: number;
  providers: { id: string; model: string; configured: boolean }[];
};
function App() {
  const [status, setStatus] = useState<Status | null>(null),
    [offline, setOffline] = useState(false),
    [tab, setTab] = useState("overview"),
    [model, setModel] = useState("space-bunny"),
    [prompt, setPrompt] = useState("用三句话介绍你自己。"),
    [key, setKey] = useState(""),
    [result, setResult] = useState(""),
    [running, setRunning] = useState(false),
    [copied, setCopied] = useState(false);
  const [draftKeys, setDraftKeys] = useState<Record<string, string>>({});
  const [clearKeys, setClearKeys] = useState<Record<string, boolean>>({});
  const [preferred, setPreferred] = useState("");
  const [saving, setSaving] = useState(false);
  const [settingsMessage, setSettingsMessage] = useState("");
  const [gatewayKey, setGatewayKey] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyMessage, setKeyMessage] = useState("");
  const [keyVisible, setKeyVisible] = useState(false);
  async function loadGatewayKey(generate = false) {
    setKeyBusy(true);
    setKeyMessage("");
    try {
      const response = await fetch("/api/gateway-key", {
        method: generate ? "POST" : "GET",
        headers: {
          "X-Gateway-Settings": "1",
          Authorization: `Bearer ${key || "local"}`,
        },
        cache: "no-store",
      });
      const data = await response.json();
      if (!response.ok) throw Error(data.error?.message || "操作失败");
      setGatewayKey(data.key);
      setKeyMessage(
        generate
          ? "新密钥已生效，旧密钥已失效。请更新其他应用的配置。"
          : data.key
            ? "密钥已加载。"
            : "尚未生成网关密钥，请点击生成。",
      );
      await refresh();
    } catch (e) {
      setKeyMessage(`操作失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setKeyBusy(false);
    }
  }
  async function copyConnection(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setKeyMessage("已复制。");
    } catch {
      setKeyVisible(true);
      setKeyMessage("复制失败，请选中文本手动复制。");
    }
  }
  async function saveSettings() {
    setSaving(true);
    setSettingsMessage("");
    const updates: Record<string, string | null> = {};
    for (const id of ["openrouter", "opencode"]) {
      if (clearKeys[id]) updates[id] = null;
      else if (draftKeys[id]?.trim()) updates[id] = draftKeys[id].trim();
    }
    if (preferred) updates.default_provider = preferred;
    try {
      const response = await fetch("/api/settings", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Gateway-Settings": "1",
          Authorization: `Bearer ${key || "local"}`,
        },
        body: JSON.stringify(updates),
      });
      const data = await response.json();
      if (!response.ok) throw Error(data.error?.message || "保存失败");
      setDraftKeys({});
      setClearKeys({});
      setPreferred("");
      setSettingsMessage("设置已保存，立即生效。重启后仍然保留。");
      await refresh();
    } catch (e) {
      setSettingsMessage(
        `保存失败：${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setSaving(false);
    }
  }
  const base = import.meta.env?.DEV
    ? "http://127.0.0.1:8787/v1"
    : `${location.origin}/v1`;
  const refresh = () =>
    fetch("/api/status")
      .then((r) => {
        if (!r.ok) throw Error();
        return r.json();
      })
      .then((s) => {
        setStatus(s);
        setOffline(false);
      })
      .catch(() => setOffline(true));
  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => clearInterval(timer);
  }, []);
  const snippet = `curl ${base}/chat/completions \\\n  -H "Authorization: Bearer ${status?.auth_required ? "YOUR_GATEWAY_KEY" : "local"}" \\\n  -H "Content-Type: application/json" \\\n  -d '{"model":"${model}","messages":[{"role":"user","content":"你好"}],"stream":true}'`;
  async function run() {
    setRunning(true);
    setResult("");
    try {
      const r = await fetch("/v1/chat/completions", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${gatewayKey || key || "local"}`,
        },
        body: JSON.stringify({
          model,
          messages: [{ role: "user", content: prompt }],
          stream: false,
        }),
      });
      const data = await r.json();
      if (!r.ok) throw Error(data.error?.message || `HTTP ${r.status}`);
      setResult(
        data.choices?.[0]?.message?.content || JSON.stringify(data, null, 2),
      );
      refresh();
    } catch (e) {
      setResult(`调用失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setRunning(false);
    }
  }
  return (
    <div className="shell">
      <aside>
        <div className="brand">
          <span className="brand-icon">
            <Network size={22} />
          </span>
          free router<span className="version">LOCAL</span>
        </div>
        <div className="workspace">
          本地网关 <span>127.0.0.1</span>
        </div>
        <div className="nav-label">WORKSPACE</div>
        <nav>
          {[
            ["overview", "网关概览", Network],
            ["playground", "模型测试", Terminal],
            ["setup", "Settings", Settings2],
          ].map(([id, label, Icon]) => (
            <button
              aria-label={label as string}
              className={tab === id ? "active" : ""}
              onClick={() => setTab(id as string)}
              key={id as string}
            >
              {React.createElement(Icon as typeof Network, { size: 18 })}
              {label as string}
              {tab === id && <i />}
            </button>
          ))}
        </nav>
        <div className="side-bottom">
          <span className={`dot ${offline ? "bad" : ""}`} />{" "}
          {offline ? "网关未连接" : status ? "本地网关运行中" : "正在连接网关…"}
          <small>RUST ENGINE · OPENAI COMPATIBLE</small>
        </div>
      </aside>
      <main>
        <header>
          <span>
            工作空间 <b>/</b>{" "}
            {tab === "overview"
              ? "网关概览"
              : tab === "playground"
                ? "模型测试"
                : "Settings"}
          </span>
          <span className="local">
            <Radio size={14} /> 本地部署
          </span>
        </header>
        <div className="content">
          <div className="heading">
            <div>
              <div className="eyebrow">YOUR MODELS. ONE ENDPOINT.</div>
              <h1>
                {tab === "overview"
                  ? "连接模型，简化调用。"
                  : tab === "playground"
                    ? "和 Space Bunny 对话。"
                    : "几分钟，完成接入。"}
              </h1>
              <p>一个本地入口，连接 OpenRouter 与 OpenCode 的 Space Bunny。</p>
            </div>
            <button
              className="primary"
              onClick={() =>
                setTab(tab === "playground" ? "overview" : "playground")
              }
            >
              {tab === "playground" ? "返回概览" : "测试模型"}
              <ArrowUpRight size={17} />
            </button>
          </div>
          {offline && (
            <div className="notice">
              无法连接 Rust 网关。请在项目根目录执行 cargo run --manifest-path
              backend/Cargo.toml。
            </div>
          )}
          {tab === "overview" && (
            <>
              <section className="hero">
                <div>
                  <span className="pill">
                    <span className="dot" /> UNIFIED GATEWAY
                  </span>
                  <h2>
                    所有请求，
                    <br />
                    在这里汇合。
                  </h2>
                  <p>
                    保持熟悉的 OpenAI 调用方式。
                    <br />
                    切换模型，无需切换 baseURL。
                  </p>
                  <div className="endpoint">
                    <code>{base}</code>
                    <button
                      title="复制 baseURL"
                      onClick={() =>
                        navigator.clipboard.writeText(base).then(() => {
                          setCopied(true);
                          setTimeout(() => setCopied(false), 1500);
                        })
                      }
                    >
                      {copied ? <Check size={17} /> : <Copy size={17} />}
                    </button>
                  </div>
                </div>
                <div className="diagram">
                  <div className="node client">
                    <Terminal size={19} />
                    你的应用
                  </div>
                  <div className="connector" />
                  <div className="node gateway">
                    <Network size={23} />
                    <strong>Free Router</strong>
                    <small>智能路由 · 故障切换</small>
                  </div>
                  <div className="branches">
                    <div className="node upstream">
                      OpenRouter <span>↗</span>
                    </div>
                    <div className="node upstream">
                      OpenCode <span>↗</span>
                    </div>
                  </div>
                </div>
              </section>
              <div className="stats">
                <article>
                  <span>已配置上游</span>
                  <strong>
                    {status
                      ? status.providers.filter((p) => p.configured).length
                      : "—"}
                    <small>/ 2</small>
                  </strong>
                  <p>API 密钥已配置</p>
                </article>
                <article>
                  <span>请求总数</span>
                  <strong>{status?.requests ?? "—"}</strong>
                  <p>本次运行累计</p>
                </article>
                <article>
                  <span>自动切换</span>
                  <strong>{status?.fallbacks ?? "—"}</strong>
                  <p>失败后尝试备用上游</p>
                </article>
                <article>
                  <span>接口协议</span>
                  <strong className="text-stat">
                    OpenAI <Zap size={18} />
                  </strong>
                  <p>支持 SSE 流式输出</p>
                </article>
              </div>
              <div className="section-title">
                <h3>模型路由</h3>
                <span>01 AUTOMATIC · 02 DIRECT</span>
              </div>
              <div className="routes">
                {[
                  [
                    "space-bunny",
                    "自动路由",
                    "按优先顺序选择已配置上游，连接失败、429 或 5xx 时切换。",
                  ],
                  [
                    "openrouter/space-bunny",
                    "OpenRouter",
                    "stealth/space-bunny-alpha",
                  ],
                  ["opencode/space-bunny", "OpenCode Zen", "space-bunny-free"],
                ].map(([id, name, desc], i) => (
                  <div className="route-row" key={id}>
                    <button
                      className="route-select"
                      onClick={() => {
                        setModel(id);
                        setTab("playground");
                      }}
                    >
                      <div className="route-icon">
                        {i === 0 ? <Network size={20} /> : <Zap size={20} />}
                      </div>
                      <div>
                        <strong>{name}</strong>
                        <code>{id}</code>
                        <p>{desc}</p>
                      </div>
                      <span className="route-tag">
                        {i === 0 ? "AUTO" : "DIRECT"}
                      </span>
                      <ArrowUpRight size={17} />
                    </button>
                    <CopyModelId id={id} />
                  </div>
                ))}
              </div>
            </>
          )}
          {tab === "playground" && (
            <section className="panel">
              <div className="section-title">
                <h3>Playground</h3>
                <span>CHAT COMPLETIONS</span>
              </div>
              <label>
                选择模型
                <select
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                >
                  <option>space-bunny</option>
                  <option>openrouter/space-bunny</option>
                  <option>opencode/space-bunny</option>
                </select>
              </label>
              {status?.auth_required && (
                <label>
                  本地网关密钥
                  <input
                    type="password"
                    value={key}
                    onChange={(e) => setKey(e.target.value)}
                    placeholder="GATEWAY_API_KEY"
                  />
                </label>
              )}
              <label>
                你的消息
                <textarea
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                />
              </label>
              <button
                className="primary"
                disabled={running || !prompt.trim()}
                onClick={run}
              >
                <Play size={16} />
                {running ? "正在生成…" : "发送请求"}
              </button>
              <div className="output">
                <span>RESPONSE</span>
                <pre>{result || "模型的回复将在这里显示。"}</pre>
              </div>
            </section>
          )}
          {tab === "setup" && (
            <>
              <section className="panel">
                <h3>统一网关 API Key</h3>
                <p>
                  将 Base URL、API Key 和模型名称填入其他应用的 OpenAI
                  兼容配置。此密钥用于模型调用，与下方上游密钥独立。
                </p>
                <div className="gateway-connection">
                  <label>
                    Base URL
                    <input readOnly value={base} />
                  </label>
                  <button onClick={() => copyConnection(base)}>
                    复制 Base URL
                  </button>
                  <label>
                    API Key
                    <input
                      readOnly
                      type={keyVisible ? "text" : "password"}
                      value={gatewayKey}
                      placeholder="点击加载或生成密钥"
                      autoComplete="off"
                    />
                  </label>
                  <div className="gateway-actions">
                    <button
                      disabled={keyBusy || offline || !status}
                      onClick={() => loadGatewayKey()}
                    >
                      加载已有密钥
                    </button>
                    <button
                      disabled={!gatewayKey}
                      onClick={() => setKeyVisible(!keyVisible)}
                    >
                      {keyVisible ? "隐藏" : "显示"}
                    </button>
                    <button
                      disabled={!gatewayKey}
                      onClick={() => copyConnection(gatewayKey)}
                    >
                      <Copy size={14} /> 复制 API Key
                    </button>
                    <button
                      disabled={keyBusy || offline || !status}
                      onClick={() => {
                        if (
                          !status?.auth_required ||
                          window.confirm(
                            "重新生成后，旧密钥将立即失效。确定继续？",
                          )
                        )
                          loadGatewayKey(true);
                      }}
                    >
                      {keyBusy
                        ? "处理中…"
                        : gatewayKey
                          ? "重新生成密钥"
                          : "生成 API Key"}
                    </button>
                  </div>
                  <p>
                    模型：<code>space-bunny</code>{" "}
                    <CopyModelId id="space-bunny" /> ·
                    密钥保存在本机，重启后仍可使用。生成后，模型接口启用密钥认证。
                  </p>
                  {keyMessage && <p role="status">{keyMessage}</p>}
                </div>
                <h3>上游 API Keys</h3>
                <p>
                  密钥保存在本机后端，保存后立即生效。留空保留已有密钥，勾选清除可移除密钥。
                </p>
                {status?.management_auth_required && (
                  <label>
                    网关管理密钥（环境变量）
                    <input
                      type="password"
                      autoComplete="off"
                      value={key}
                      onChange={(e) => setKey(e.target.value)}
                      placeholder="输入 GATEWAY_API_KEY 以保存设置"
                    />
                  </label>
                )}
                {(["openrouter", "opencode"] as const).map((id) => {
                  const configured = status?.providers.find(
                    (p) => p.id === id,
                  )?.configured;
                  return (
                    <div className="settings-provider" key={id}>
                      <div className="provider-status">
                        <b>
                          {id === "openrouter" ? "OpenRouter" : "OpenCode Zen"}
                        </b>
                        <span className={configured ? "ready" : ""}>
                          {configured ? "已保存密钥" : "未配置密钥"}
                        </span>
                      </div>
                      <label>
                        {id === "openrouter"
                          ? "OpenRouter API Key"
                          : "OpenCode API Key"}
                        <input
                          type="password"
                          autoComplete="new-password"
                          value={draftKeys[id] || ""}
                          disabled={saving || clearKeys[id]}
                          placeholder={
                            configured
                              ? "输入新密钥以替换；留空保留"
                              : "粘贴 API Key"
                          }
                          onChange={(e) =>
                            setDraftKeys({ ...draftKeys, [id]: e.target.value })
                          }
                        />
                      </label>
                      {configured && (
                        <label className="clear-key">
                          <input
                            type="checkbox"
                            disabled={saving}
                            checked={!!clearKeys[id]}
                            onChange={(e) =>
                              setClearKeys({
                                ...clearKeys,
                                [id]: e.target.checked,
                              })
                            }
                          />{" "}
                          清除此上游密钥
                        </label>
                      )}
                    </div>
                  );
                })}
                <label>
                  自动路由优先上游
                  <select
                    disabled={saving}
                    value={preferred || status?.default_provider || "opencode"}
                    onChange={(e) => setPreferred(e.target.value)}
                  >
                    <option value="opencode">OpenCode Zen</option>
                    <option value="openrouter">OpenRouter</option>
                  </select>
                </label>
                <button
                  className="primary"
                  disabled={saving || offline || !status}
                  onClick={saveSettings}
                >
                  <Check size={16} />
                  {saving ? "保存中…" : "保存设置"}
                </button>
                {settingsMessage && (
                  <p
                    role="status"
                    className={
                      settingsMessage.startsWith("保存失败")
                        ? "notice"
                        : "save-success"
                    }
                  >
                    {settingsMessage}
                  </p>
                )}
              </section>
              <UpdateSettings gatewayKey={key} />
            </>
          )}
          <section className="integration">
            <div className="section-title">
              <h3>
                <Terminal size={18} /> 开始调用
              </h3>
              <button
                onClick={() =>
                  navigator.clipboard.writeText(snippet).then(() => {
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1500);
                  })
                }
              >
                {copied ? <Check size={15} /> : <Copy size={15} />}{" "}
                {copied ? "已复制" : "复制代码"}
              </button>
            </div>
            <div className="selected-model">
              <span>模型 ID</span>
              <code>{model}</code>
              <CopyModelId id={model} />
            </div>
            <pre>{snippet}</pre>
            <div className="hint">
              <span className="dot" /> 可用于 OpenAI SDK、聊天客户端和支持自定义
              baseURL 的工具
            </div>
          </section>
          <footer>
            FREE ROUTER <span>本地连接，无限可能。</span>
            <span>v0.1.0</span>
          </footer>
        </div>
      </main>
    </div>
  );
}
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
