import { ArrowUpRight, Check, Copy } from "lucide-react";
import UpdateSettings from "./UpdateSettings";
import CopyModelId from "./CopyModelId";
import type { Status } from "./status";
import type { GatewaySettingsState } from "./useGatewaySettings";
import { DEFAULT_MODEL_ID } from "./piGateway";
const PROVIDER_NAMES: Record<string, string> = { openrouter: "OpenRouter" };
const PROVIDER_KEY_URLS: Record<string, string> = { openrouter: "https://openrouter.ai/settings/keys" };
export default function SettingsPanel({ settings, status, base, managementKey: key, onKeyChange: setKey, offline }: {
  settings: GatewaySettingsState; status: Status | null; base: string;
  managementKey: string; onKeyChange: (key: string) => void; offline: boolean;
}) {
  const { draftKeys, setDraftKeys, removedKeys, setRemovedKeys, clearKeys, setClearKeys, saving, settingsMessage, exaKey, setExaKey, clearExa, setClearExa, gatewayKey, keyBusy, keyMessage, keyVisible, setKeyVisible, loadGatewayKey, copyConnection, saveSettings } = settings;
  return (
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
                    模型：<code>{DEFAULT_MODEL_ID}</code>{" "}
                    <CopyModelId id={DEFAULT_MODEL_ID} /> ·
                    密钥保存在本机，重启后仍可使用。生成后，模型接口启用密钥认证。
                  </p>
                  {keyMessage && <p role="status">{keyMessage}</p>}
                </div>
                <h3>上游 API Keys</h3>
                <p>
                  每个上游最多保存 16 个 API
                  Key，按请求轮询。新增或删除后点击保存设置，立即生效。
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
                {(["openrouter"] as const).map((id) => {
                  const provider = status?.providers.find((p) => p.id === id);
                  const configured = provider?.configured;
                  const drafts = draftKeys[id] || [""];
                  const removed = removedKeys[id] || [];
                  return (
                    <div className="settings-provider" key={id}>
                      <div className="provider-status">
                        <b>
                          {PROVIDER_NAMES[id]}
                        </b>
                        <span className={configured ? "ready" : ""}>
                          {configured
                            ? `${provider?.key_count} 个密钥 · 轮询`
                            : "未配置密钥"}
                        </span>
                      </div>
                      <a
                        className="get-api-key"
                        href={
                          PROVIDER_KEY_URLS[id]
                        }
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label={`获取 ${PROVIDER_NAMES[id]} API Key（新窗口）`}
                      >
                        获取 API Key <ArrowUpRight size={14} />
                      </a>
                      <span className="key-link-hint">
                        登录官方控制台创建密钥
                      </span>
                      {provider?.keys?.map((entry) => (
                        <div
                          className={`saved-key ${removed.includes(entry.id) || clearKeys[id] ? "pending-removal" : ""}`}
                          key={entry.id}
                        >
                          <span>
                            {entry.label} ·{" "}
                            {removed.includes(entry.id) || clearKeys[id]
                              ? "待删除"
                              : entry.retry_after_seconds
                                ? `冷却中 · ${entry.retry_after_seconds} 秒后重试`
                                : "已保存"}
                          </span>
                          <button
                            type="button"
                            disabled={saving || clearKeys[id]}
                            onClick={() =>
                              setRemovedKeys({
                                ...removedKeys,
                                [id]: removed.includes(entry.id)
                                  ? removed.filter((k) => k !== entry.id)
                                  : [...removed, entry.id],
                              })
                            }
                          >
                            {removed.includes(entry.id) ? "撤销删除" : "删除"}
                          </button>
                        </div>
                      ))}
                      {drafts.map((draft, index) => (
                        <div className="key-draft" key={index}>
                          <label>
                            {PROVIDER_NAMES[id]} 新
                            API Key {index + 1}
                            <input
                              type="password"
                              autoComplete="new-password"
                              value={draft}
                              disabled={saving || clearKeys[id]}
                              placeholder="粘贴新增的 API Key；留空不修改"
                              onChange={(e) =>
                                setDraftKeys({
                                  ...draftKeys,
                                  [id]: drafts.map((v, i) =>
                                    i === index ? e.target.value : v,
                                  ),
                                })
                              }
                            />
                          </label>
                          {drafts.length > 1 && (
                            <button
                              type="button"
                              className="secondary"
                              disabled={saving || clearKeys[id]}
                              aria-label={`移除 ${id} 新 API Key ${index + 1}`}
                              onClick={() =>
                                setDraftKeys({
                                  ...draftKeys,
                                  [id]: drafts.filter((_, i) => i !== index),
                                })
                              }
                            >
                              移除
                            </button>
                          )}
                        </div>
                      ))}
                      <button
                        type="button"
                        className="secondary"
                        disabled={
                          saving ||
                          clearKeys[id] ||
                          (provider?.key_count || 0) -
                            removed.length +
                            drafts.length >=
                            16
                        }
                        onClick={() =>
                          setDraftKeys({ ...draftKeys, [id]: [...drafts, ""] })
                        }
                      >
                        + 添加 API Key
                      </button>
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
                          清除此上游全部密钥
                        </label>
                      )}
                    </div>
                  );
                })}
                <div className="settings-provider">
                  <div className="provider-status"><b>Exa Web Search</b><span className={status?.exa_configured ? "ready" : ""}>{status?.exa_configured ? "已配置" : "未配置密钥"}</span></div>
                  <a className="get-api-key" href="https://dashboard.exa.ai/api-keys" target="_blank" rel="noopener noreferrer">获取 Exa API Key <ArrowUpRight size={14} /></a>
                  <p>供 Pi Agent 的 web_search 工具搜索网页，返回标题、链接与内容摘要。</p>
                  <label>Exa API Key<input type="password" autoComplete="new-password" value={exaKey} disabled={saving || clearExa} onChange={e => setExaKey(e.target.value)} placeholder="粘贴 API Key；留空保留原密钥" /></label>
                  {status?.exa_configured && <label className="clear-key"><input type="checkbox" disabled={saving} checked={clearExa} onChange={e => setClearExa(e.target.checked)} /> 清除 Exa 密钥</label>}
                </div>
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
              {window.freeRouterDesktop
                ? <section className="panel"><h3>桌面应用更新</h3><p>下载并安装新版本桌面安装包。配置保存在应用数据目录，升级后保留。</p><a className="get-api-key" href="https://github.com/Neonity2020/free-router/releases" target="_blank" rel="noopener noreferrer">GitHub Releases <ArrowUpRight size={14} /></a></section>
                : <UpdateSettings gatewayKey={key} />}
            </>
  );
}
