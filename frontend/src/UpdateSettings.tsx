import { useEffect, useState } from "react";
import { Download, RefreshCw, Check, Github } from "lucide-react";

type UpdateStatus = {
  current_version: string;
  platform: string;
  config: { repository: string; auto_download: boolean };
  phase: string;
  latest_version: string | null;
  checked_at: number | null;
  downloaded: number;
  total: number;
  package_path: string | null;
  error: string | null;
};
const labels: Record<string, string> = {
  idle: "等待检查",
  checking: "正在检查版本…",
  available: "发现新版本",
  up_to_date: "已是最新版本",
  downloading: "正在下载更新…",
  ready: "下载完成 · 待安装",
  error: "更新失败",
};
export default function UpdateSettings({ gatewayKey }: { gatewayKey: string }) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [repository, setRepository] = useState<string | null>(null);
  const [automatic, setAutomatic] = useState<boolean | null>(null);
  const [message, setMessage] = useState("");
  const [requesting, setRequesting] = useState(false);
  const [connectionError, setConnectionError] = useState("");
  useEffect(() => {
    let cancelled = false;
    async function refresh() {
      try {
        const response = await fetch("/api/updates", {
          headers: { Authorization: `Bearer ${gatewayKey || "local"}` },
        });
        const data = await response.json();
        if (!response.ok)
          throw Error(
            response.status === 401
              ? "请输入本地网关密钥以管理更新。"
              : "无法读取更新状态",
          );
        if (!cancelled) {
          setStatus(data);
          setConnectionError("");
        }
      } catch (error) {
        if (!cancelled)
          setConnectionError(
            error instanceof Error ? error.message : "无法连接网关",
          );
      }
    }
    refresh();
    const timer = setInterval(refresh, 1500);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [gatewayKey]);
  const busy =
    requesting ||
    status?.phase === "checking" ||
    status?.phase === "downloading";
  async function action(kind: "settings" | "check" | "download") {
    setRequesting(true);
    setMessage("");
    try {
      const response = await fetch(`/api/updates/${kind}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Gateway-Settings": "1",
          Authorization: `Bearer ${gatewayKey || "local"}`,
        },
        ...(kind === "settings"
          ? {
              body: JSON.stringify({
                repository: (
                  repository ??
                  status?.config.repository ??
                  ""
                ).trim(),
                auto_download:
                  automatic ?? status?.config.auto_download ?? true,
              }),
            }
          : {}),
      });
      const data = await response.json();
      if (!response.ok) throw Error(data.error?.message || "操作失败");
      // Fetch saved config before clearing drafts, avoiding a stale polling snapshot.
      const refreshed = await fetch("/api/updates", {
        headers: { Authorization: `Bearer ${gatewayKey || "local"}` },
      });
      if (refreshed.ok) {
        setStatus(await refreshed.json());
        if (kind === "settings") {
          setRepository(null);
          setAutomatic(null);
        }
      }
      setMessage(
        kind === "settings"
          ? "更新设置已保存。仓库配置后会立即检查版本。"
          : "更新任务已启动。",
      );
    } catch (error) {
      setMessage(
        `操作失败：${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      setRequesting(false);
    }
  }
  const percent = status?.total
    ? Math.min(100, Math.floor((status.downloaded / status.total) * 100))
    : 0;
  return (
    <section className="panel update-panel">
      <div className="section-title">
        <h3>
          <Download size={18} /> 应用更新
        </h3>
        <span>GITHUB RELEASES</span>
      </div>
      <p>
        启动时和每 6
        小时检查正式版本，自动下载当前平台的更新包。下载完成后由你安排安装。
      </p>
      {connectionError && (
        <p className="notice" role="alert">
          {connectionError}
        </p>
      )}
      <div className="update-meta">
        <span>
          当前版本 <b>v{status?.current_version ?? "—"}</b>
        </span>
        <span>
          平台 <code>{status?.platform ?? "—"}</code>
        </span>
      </div>
      <label>
        GitHub 仓库
        <input
          placeholder="owner/free-router"
          value={repository ?? status?.config.repository ?? ""}
          disabled={!!busy}
          onChange={(e) => setRepository(e.target.value)}
          autoComplete="off"
        />
      </label>
      <label className="clear-key">
        <input
          type="checkbox"
          checked={automatic ?? status?.config.auto_download ?? true}
          disabled={!!busy}
          onChange={(e) => setAutomatic(e.target.checked)}
        />{" "}
        发现新版本时自动下载
      </label>
      <div className="update-actions">
        <button
          className="primary"
          onClick={() => action("settings")}
          disabled={!!busy || !status || !!connectionError}
        >
          <Check size={16} />
          保存更新设置
        </button>
        <button
          className="secondary"
          onClick={() => action("check")}
          disabled={!!busy || !status?.config.repository || !!connectionError}
        >
          <RefreshCw size={16} />
          检查更新
        </button>
        {status?.latest_version && status.phase !== "ready" && (
          <button
            className="secondary"
            onClick={() => action("download")}
            disabled={!!busy || !!connectionError}
          >
            <Download size={16} />
            下载更新
          </button>
        )}
      </div>
      {message && (
        <p
          role="status"
          className={message.startsWith("操作失败") ? "notice" : "save-success"}
        >
          {message}
        </p>
      )}
      <div className="update-state" aria-live="polite">
        <span className={`dot ${status?.phase === "error" ? "bad" : ""}`} />
        <b>
          {status?.config.repository
            ? labels[status.phase] || status.phase
            : "配置仓库后启用更新"}
        </b>
        {status?.latest_version && <span>v{status.latest_version}</span>}
        {status?.checked_at && (
          <small>
            上次检查：{new Date(status.checked_at * 1000).toLocaleString()}
          </small>
        )}
      </div>
      {status?.phase === "downloading" && (
        <div className="download-progress">
          <progress value={status.downloaded} max={status.total || 1} />
          <span>
            {percent}% · {(status.downloaded / 1024 / 1024).toFixed(1)} /{" "}
            {(status.total / 1024 / 1024).toFixed(1)} MiB
          </span>
        </div>
      )}
      {status?.error && (
        <p className="notice" role="alert">
          {status.error}
        </p>
      )}
      {status?.package_path && (
        <div className="update-ready">
          <strong>
            <Check size={16} />
            SHA-256 校验通过
          </strong>
          <p>更新包已保存在本机：</p>
          <code>{status.package_path}</code>
          <p>
            解压到新目录，停止旧网关，将密钥和更新配置文件复制到新目录后运行
            ./free-router。
          </p>
        </div>
      )}
      {status?.config.repository && (
        <a
          className="release-link"
          href={`https://github.com/${status.config.repository}/releases`}
          target="_blank"
          rel="noreferrer"
        >
          <Github size={15} />
          查看 Releases
        </a>
      )}
    </section>
  );
}
