import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";

export default function CopyModelId({ id }: { id: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  useEffect(() => {
    setState("idle");
  }, [id]);
  useEffect(() => {
    if (state !== "copied") return;
    const timer = setTimeout(() => setState("idle"), 2000);
    return () => clearTimeout(timer);
  }, [state]);
  return (
    <span className="model-copy-control">
      <button
        type="button"
        className="copy-model"
        aria-label={`复制模型 ID ${id}`}
        title={`复制 ${id}`}
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(id);
            setState("copied");
          } catch {
            setState("failed");
          }
        }}
      >
        {state === "copied" ? <Check size={14} /> : <Copy size={14} />}
        <span aria-live="polite">
          {state === "copied" ? "已复制" : "复制 ID"}
        </span>
      </button>
      {state === "failed" && (
        <span className="copy-failed" role="status">
          复制失败，请手动复制：<code>{id}</code>
        </span>
      )}
    </span>
  );
}
