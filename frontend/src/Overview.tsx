import { useState } from "react";
import { ArrowUpRight, Check, Copy, Network, Terminal, Zap } from "lucide-react";
import CopyModelId from "./CopyModelId";
import type { Status } from "./status";
import { DEFAULT_MODEL_ID } from "./piGateway";
export default function Overview({ status, base, onSelectModel }: { status: Status | null; base: string; onSelectModel: (id: string) => void }) {
  const [copied, setCopied] = useState(false);
  return (
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
                    <small>免费模型 · 密钥轮询</small>
                  </div>
                  <div className="branches">
                    <div className="node upstream">
                      OpenRouter <span>↗</span>
                    </div>
                  </div>
                </div>
              </section>
              <div className="stats">
                <article>
                  <span>已配置上游</span>
                  <strong>
                    {status
                      ? status.providers.filter((p) => p.id === "openrouter" && p.configured).length
                      : "—"}
                    <small>/ 1</small>
                  </strong>
                  <p>API 密钥已配置</p>
                </article>
                <article>
                  <span>请求总数</span>
                  <strong>{status?.requests ?? "—"}</strong>
                  <p>本次运行累计</p>
                </article>
                <article>
                  <span>可用模型路由</span>
                  <strong>2</strong>
                  <p>OpenRouter 免费模型</p>
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
                <span>02 OPENROUTER ROUTES</span>
              </div>
              <div className="routes">
                {[
                  [
                    DEFAULT_MODEL_ID,
                    "OpenRouter",
                    "根据请求能力自动选择可用的免费模型。",
                  ],
                  ["openrouter/apodex/apodex-1.1-mini:free", "OpenRouter", "apodex/apodex-1.1-mini:free"],
                ].map(([id, name, desc], i) => (
                  <div className="route-row" key={id}>
                    <button
                      className="route-select"
                      onClick={() => {
                        onSelectModel(id);
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
  );
}
