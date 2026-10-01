import { useState } from "react";
import { ChevronRight, Brain } from "lucide-react";
import { compactTools, type Item } from "./agentTimeline";
import MarkdownMessage from "./MarkdownMessage";

export default function ToolActivity({ items }: { items: Item[] }) {
  const [expanded, setExpanded] = useState(false);
  const running = items.some(item => item.status === "running");
  const failures = items.filter(item => item.status === "error").length;
  const stopped = items.some(item => item.status === "stopped");
  return <section className={`agent-activity${running ? " running" : ""}${failures ? " error" : ""}`}>
    <button type="button" className="agent-activity-toggle" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
      <ChevronRight size={14} className={expanded ? "expanded" : ""} /><Brain size={14} />
      <span>{running ? "正在思考" : stopped ? "思考已停止" : "思考过程"}</span><small>{failures > 0 && `${failures} 次工具调用失败`}</small>
    </button>
    {expanded && <div className="agent-activity-details">{compactTools(items).map(({ item, count }) =>
      item.type === "assistant" ? <div className="agent-thinking" key={item.id}><MarkdownMessage text={item.thinking || ""} /></div> : <details className={`agent-tool ${item.status}`} key={item.id}>
        <summary>{item.name}{count > 1 && <b>相同调用与输出 × {count}</b>}<span>{item.status === "running" ? "执行中" : item.status === "error" ? "失败" : item.status === "stopped" ? "已停止" : "完成"}</span></summary>
        <pre aria-label="工具参数">{item.args}</pre>{item.text && <pre aria-label="工具输出">{item.text}</pre>}
      </details>)}</div>}
  </section>;
}
