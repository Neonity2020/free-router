import { useEffect, useRef, useState } from "react";
import { Bot, FolderOpen, Plus, Send, Square, Terminal, Trash2 } from "lucide-react";
import { DEFAULT_MODEL_ID, PI_SDK_VERSION } from "./piGateway";
import MarkdownMessage from "./MarkdownMessage";
import ToolActivity from "./ToolActivity";
import { buildTimeline, type Item } from "./agentTimeline";
type SessionSummary = { id: string; cwd: string; model: string; busy: boolean; title: string };
type Session = SessionSummary & { tools: string[]; items: Item[] };

export default function PiAgent({ managementRequired, gatewayKey, onKeyChange }: {
  managementRequired: boolean; gatewayKey: string; onKeyChange: (key: string) => void;
}) {
  const [cwd, setCwd] = useState("");
  const [model, setModel] = useState<string>(DEFAULT_MODEL_ID);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [activeId, setActiveId] = useState("");
  const [session, setSession] = useState<Session | null>(null);
  const [prompt, setPrompt] = useState("");
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState(false);
  const [choosingDirectory, setChoosingDirectory] = useState(false);
  const [loading, setLoading] = useState(true);
  const [reload, setReload] = useState(0);
  const transcript = useRef<HTMLDivElement>(null);
  const followOutput = useRef(true);

  async function request(path: string, method = "GET", body?: unknown) {
    const response = await fetch(`/api/agent/${path}`, {
      method, cache: "no-store", headers: {
        "Content-Type": "application/json", "X-Gateway-Settings": "1",
        Authorization: `Bearer ${gatewayKey || "local"}`,
      }, body: body === undefined ? undefined : JSON.stringify(body),
    });
    const data = await response.json();
    if (!response.ok) throw Object.assign(new Error(data.error?.message || `HTTP ${response.status}`), { status: response.status });
    return data;
  }
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setReady(false);
    if (managementRequired && !gatewayKey) { setLoading(false); return; }
    Promise.all([request("status"), request("sessions")]).then(([status, data]) => {
      if (cancelled) return;
      setCwd(previous => previous || status.default_cwd);
      setSessions(data.sessions);
      setActiveId(previous => data.sessions.some((s: SessionSummary) => s.id === previous)
        ? previous : data.sessions.at(-1)?.id || "");
      setReady(true); setError("");
    }).catch(e => { if (!cancelled) setError(e.message); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [gatewayKey, managementRequired, reload]);

  useEffect(() => {
    if (!activeId || !ready) { setSession(null); return; }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    setSession(null);
    const poll = async () => {
      try {
        const [data, list] = await Promise.all([request(`sessions/${activeId}`), request("sessions")]);
        if (!cancelled) { setSession(data); setSessions(list.sessions); }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
          if (e instanceof Error && "status" in e && e.status === 404) {
            setActiveId(""); setSession(null);
            try {
              const list = await request("sessions");
              if (!cancelled) setSessions(list.sessions);
            } catch { if (!cancelled) setSessions([]); }
          }
        }
      }
      if (!cancelled) timer = setTimeout(poll, 700);
    };
    poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [activeId, ready, gatewayKey]);

  const outputLength = session?.items.reduce((sum, item) => sum + item.text.length + (item.thinking?.length || 0), 0);
  useEffect(() => {
    const element = transcript.current;
    if (element && followOutput.current) element.scrollTop = element.scrollHeight;
  }, [outputLength, session?.items.length]);

  async function create() {
    setPending(true); setError("");
    try {
      const data: Session = await request("sessions", "POST", { cwd: cwd.trim(), model });
      setSessions(previous => [...previous, data]); setActiveId(data.id); setPrompt("");
      followOutput.current = true;
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setPending(false); }
  }
  async function chooseDirectory() {
    setChoosingDirectory(true); setError("");
    try {
      const data = window.freeRouterDesktop
        ? await window.freeRouterDesktop.chooseDirectory(cwd.trim())
        : await request("pick-directory", "POST", { cwd: cwd.trim() });
      if (!data.cancelled && typeof data.cwd === "string") setCwd(data.cwd);
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setChoosingDirectory(false); }
  }
  async function send() {
    if (!session || !prompt.trim() || session.busy || pending) return;
    setPending(true); setError("");
    try {
      const data = await request(`sessions/${session.id}/prompt`, "POST", { prompt });
      setSession(data); setPrompt(""); followOutput.current = true;
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setPending(false); }
  }
  async function stop() {
    if (!session) return;
    setPending(true);
    try { setSession(await request(`sessions/${session.id}/abort`, "POST")); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setPending(false); }
  }
  async function remove() {
    if (!session) return;
    setPending(true);
    try {
      await request(`sessions/${session.id}`, "DELETE");
      setSessions(previous => previous.filter(s => s.id !== session.id));
      setActiveId(""); setSession(null);
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setPending(false); }
  }
  const busy = session?.busy || sessions.some(s => s.busy);
  return (
    <section className="pi-agent">
      <div className="agent-config panel">
        <div className="section-title"><h3><Bot size={18} /> Pi Agent</h3><span>SDK {PI_SDK_VERSION}</span></div>
        <p>连接已配置的网关，让 Pi 阅读代码、修改文件并运行命令。</p>
        {managementRequired && <label>网关管理密钥
          <input type="password" autoComplete="off" value={gatewayKey} onChange={e => onKeyChange(e.target.value)} placeholder="GATEWAY_API_KEY" />
        </label>}
        <label>工作目录（绝对路径）
          <div className="agent-directory-picker">
            <input value={cwd} onChange={e => setCwd(e.target.value)} disabled={choosingDirectory} placeholder="/Users/you/projects/my-app" />
            <button type="button" className="secondary" disabled={choosingDirectory || (managementRequired && !gatewayKey)} onClick={chooseDirectory}>
              <FolderOpen size={16} />{choosingDirectory ? "选择中…" : window.freeRouterDesktop ? "选择文件夹" : "Finder 选择"}
            </button>
          </div>
        </label>
        <label>模型路由
          <select value={model} onChange={e => setModel(e.target.value)}>
            <option>{DEFAULT_MODEL_ID}</option>
          </select>
        </label>
        <p className="agent-permissions">工具以本机用户权限读写文件、执行命令。工作目录不是沙箱；新建会话后即可执行编程任务。</p>
        <button className="primary" onClick={create} disabled={!ready || !cwd.trim() || pending || busy || choosingDirectory}>
          <Plus size={16} /> {loading ? "连接 Agent…" : "新建编程会话"}
        </button>
        {!ready && !loading && <button className="secondary" onClick={() => setReload(value => value + 1)}>重试连接</button>}
        <div className="agent-session-list" aria-label="编程会话">
          {[...sessions].reverse().map(s => <button key={s.id} className={s.id === activeId ? "active" : ""}
            disabled={pending} onClick={() => { setActiveId(s.id); followOutput.current = true; }}>
            <span>{s.busy ? "● " : ""}{s.title}</span><small>{s.cwd}</small>
          </button>)}
        </div>
        <small>会话在当前服务运行期间保留，刷新页面可继续；重启后会话清空，文件修改保留。</small>
      </div>
      <div className="agent-chat panel">
        <div className="agent-chat-header">
          <div><strong>{session ? session.title : "开始一次编程任务"}</strong>
            <p><FolderOpen size={14} /> {session?.cwd || "先选择目录并新建会话"}</p></div>
          {session && <button className="secondary" aria-label="删除编程会话" title="删除会话记录，保留文件修改"
            disabled={session.busy || pending} onClick={remove}><Trash2 size={16} /></button>}
        </div>
        {error && <div className="notice" role="alert">{error}</div>}
        <div className="agent-transcript" ref={transcript} onScroll={e => {
          const element = e.currentTarget;
          followOutput.current = element.scrollHeight - element.scrollTop - element.clientHeight < 100;
        }} aria-label="Agent 对话与工具执行">
          {!session?.items.length && <div className="agent-empty"><Terminal size={30} />
            <h3>从代码到可运行的结果</h3><p>例如：阅读当前项目，修复登录失败的问题并运行相关测试。</p>
            <div>read · write · edit · bash · grep · find · ls · web_search</div>
          </div>}
          {buildTimeline(session?.items || []).map(entry => {
            if (entry.kind === "tools") return <ToolActivity key={`${activeId}:${entry.id}`} items={entry.items} />;
            const item = entry.item;
            return (
            <article className={`agent-message ${item.type}${item.error ? " error" : ""}`} key={item.id}>
              <small>{item.type === "user" ? "你" : item.type === "assistant" ? "PI AGENT" : "运行状态"}</small>
              {item.thinking && <details><summary>思考过程</summary><MarkdownMessage text={item.thinking} /></details>}
              {item.type === "assistant"
                ? <MarkdownMessage text={item.text} />
                : <pre>{item.text}</pre>}
            </article>
          ); })}
          {session?.busy && !session.items.some(item => item.status === "running") && <p className="agent-working" role="status">正在思考…</p>}
        </div>
        <form className="agent-composer" onSubmit={e => { e.preventDefault(); send(); }}>
          <label>编程任务<textarea value={prompt} onChange={e => setPrompt(e.target.value)}
            placeholder="描述需要实现或修复的问题…" disabled={!session || session.busy || pending} /></label>
          <div className="agent-composer-actions"><small>{session?.busy ? "Pi 正在执行任务…" : session ? `${session.model} · 支持连续对话` : "新建会话后开始"}</small>
            {session?.busy ? <button type="button" className="secondary" disabled={pending} onClick={stop}><Square size={14} />停止任务</button>
              : <button className="primary" type="submit" disabled={!session || !prompt.trim() || pending || busy}><Send size={15} />发送任务</button>}
          </div>
        </form>
      </div>
    </section>
  );
}
