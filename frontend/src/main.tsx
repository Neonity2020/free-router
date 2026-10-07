import React, { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  ArrowUpRight,
  Bot,
  Check,
  Copy,
  Network,
  Play,
  Radio,
  Settings2,
  Terminal,
} from "lucide-react";
import "./style.css";
import SettingsPanel from "./SettingsPanel";
import { useGatewaySettings } from "./useGatewaySettings";
import ThemeSwitcher from "./ThemeSwitcher";
import PiAgent from "./PiAgent";
import MarkdownMessage from "./MarkdownMessage";
import { DEFAULT_MODEL_ID, GATEWAY_MODEL_IDS, PI_SDK_VERSION, gatewayPiSnippet, streamGatewayReply } from "./piGateway";
import type { Status } from "./status";
import Overview from "./Overview";
import CopyModelId from "./CopyModelId";
function App() {
  const [status, setStatus] = useState<Status | null>(null),
    [offline, setOffline] = useState(false),
    [tab, setTab] = useState(() => ["overview", "playground", "agent", "setup"].includes(location.hash.slice(1))
      ? location.hash.slice(1) : "overview"),
    [model, setModel] = useState<string>(DEFAULT_MODEL_ID),
    [prompt, setPrompt] = useState("用三句话介绍你自己。"),
    [key, setKey] = useState(""),
    [result, setResult] = useState(""),
    [running, setRunning] = useState(false),
    [copied, setCopied] = useState(false);
  const requestController = useRef<AbortController | null>(null);
  const [thinking, setThinking] = useState("");
  const [requestMessage, setRequestMessage] = useState("");
  const [usage, setUsage] = useState<{ input: number; output: number } | null>(null);
  const [example, setExample] = useState("curl");
  useEffect(() => () => requestController.current?.abort(), []);
  useEffect(() => { history.replaceState(null, "", `#${tab}`); }, [tab]);
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
        if (window.freeRouterDesktop) setStatus({ ...s, management_auth_required: false });
        setOffline(false);
      })
      .catch(() => setOffline(true));
  const settings = useGatewaySettings(key, refresh);
  const { gatewayKey } = settings;
  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => clearInterval(timer);
  }, []);
  const snippet = `curl ${base}/chat/completions \\\n  -H "Authorization: Bearer ${status?.auth_required ? "YOUR_GATEWAY_KEY" : "local"}" \\\n  -H "Content-Type: application/json" \\\n  -d '{"model":"${model}","messages":[{"role":"user","content":"你好"}],"stream":true}'`;
  const displayedSnippet = example === "pi" ? gatewayPiSnippet(base, model) : snippet;
  async function run() {
    if (requestController.current) return;
    const controller = new AbortController();
    requestController.current = controller;
    setRunning(true);
    setResult("");
    setThinking("");
    setRequestMessage("");
    setUsage(null);
    try {
      const message = await streamGatewayReply({
        baseUrl: `${location.origin}/v1`,
        modelId: model,
        apiKey: gatewayKey || key || "local",
        prompt,
        signal: controller.signal,
        onText: setResult,
        onThinking: setThinking,
      });
      setRequestMessage(
        message.stopReason === "aborted" || controller.signal.aborted
          ? "已停止生成。"
          : message.stopReason === "length"
            ? "已达到输出长度上限。"
            : "生成完成。",
      );
      if (message.usage.totalTokens > 0) setUsage(message.usage);
    } catch (e) {
      setRequestMessage(
        controller.signal.aborted
          ? "已停止生成。"
          : `调用失败：${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      requestController.current = null;
      setRunning(false);
      refresh();
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
            ["agent", "Pi Agent", Bot],
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
                : tab === "agent"
                  ? "Pi Agent"
                : "Settings"}
          </span>
          <div className="header-actions">
            <ThemeSwitcher />
            <span className="local">
              <Radio size={14} /> 本地部署
            </span>
          </div>
        </header>
        <div className="content">
          <div className="heading">
            <div>
              <div className="eyebrow">YOUR MODELS. ONE ENDPOINT.</div>
              <h1>
                {tab === "overview"
                  ? "连接模型，简化调用。"
                  : tab === "playground"
                    ? "和 openrouter/free 对话。"
                    : tab === "agent"
                      ? "让 Pi 完成编程任务。"
                    : "几分钟，完成接入。"}
              </h1>
              <p>一个本地入口，通过 OpenRouter 调用免费模型。</p>
            </div>
            <button
              className="primary"
              onClick={() =>
                setTab(tab === "playground" || tab === "agent" ? "overview" : "playground")
              }
            >
              {tab === "playground" || tab === "agent" ? "返回概览" : "测试模型"}
              <ArrowUpRight size={17} />
            </button>
          </div>
          {offline && (
            <div className="notice">
              无法连接 Rust 网关。请在项目根目录执行 cargo run --manifest-path
              backend/Cargo.toml。
            </div>
          )}
          {tab === "overview" && <Overview status={status} base={base} onSelectModel={id => { setModel(id); setTab("playground"); }} />}
          {tab === "playground" && (
            <section className="panel">
              <div className="section-title">
                <h3>Playground</h3>
                <span>PI AI SDK · {PI_SDK_VERSION} · STREAMING</span>
              </div>
              <label>
                选择模型
                <select
                  value={model}
                  disabled={running}
                  onChange={(e) => setModel(e.target.value)}
                >
                  {GATEWAY_MODEL_IDS.map(id => <option key={id}>{id}</option>)}
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
              {running && (
                <button className="secondary" onClick={() => requestController.current?.abort()}>
                  停止生成
                </button>
              )}
              {requestMessage && <p role="status">{requestMessage}</p>}
              {usage && <p>输入 {usage.input} tokens · 输出 {usage.output} tokens</p>}
              {thinking && <details><summary>思考过程</summary><MarkdownMessage text={thinking} /></details>}
              <div className="output">
                <span>RESPONSE</span>
                <MarkdownMessage text={result || "模型的回复将在这里显示。"} />
              </div>
            </section>
          )}
          {tab === "agent" && <PiAgent managementRequired={!!status?.management_auth_required}
            gatewayKey={key} onKeyChange={setKey} />}
          {tab === "setup" && <SettingsPanel settings={settings} status={status} base={base} managementKey={key} onKeyChange={setKey} offline={offline} />}
          {tab !== "agent" && <section className="integration">
            <div className="section-title">
              <h3>
                <Terminal size={18} /> 开始调用
              </h3>
              <button
                onClick={() =>
                  navigator.clipboard.writeText(displayedSnippet).then(() => {
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1500);
                  })
                }
              >
                {copied ? <Check size={15} /> : <Copy size={15} />}{" "}
                {copied ? "已复制" : "复制代码"}
              </button>
            </div>
            <label>
              调用方式
              <select value={example} onChange={(e) => setExample(e.target.value)}>
                <option value="curl">cURL</option>
                <option value="pi">Pi AI SDK {PI_SDK_VERSION}</option>
              </select>
            </label>
            <div className="selected-model">
              <span>模型 ID</span>
              <code>{model}</code>
              <CopyModelId id={model} />
            </div>
            <pre>{displayedSnippet}</pre>
            <div className="hint">
              <span className="dot" /> 可用于 OpenAI SDK、聊天客户端和支持自定义
              baseURL 的工具
            </div>
          </section>}
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
