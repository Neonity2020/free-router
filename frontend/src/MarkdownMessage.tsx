import { Children, isValidElement, memo, useEffect, useRef, useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { Check, Copy } from "lucide-react";

function CodeBlock({ children }: { children: ReactNode }) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const child = Children.toArray(children)[0];
  const code = isValidElement<{ children?: ReactNode; className?: string }>(child) ? child : null;
  const text = typeof code?.props.children === "string" ? code.props.children : "";
  const language = code?.props.className?.match(/language-(\S+)/)?.[1] || "text";
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true); setFailed(false);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1500);
    } catch { setFailed(true); }
  }
  return (
    <div className="markdown-code">
      <div className="markdown-code-header"><span>{language}</span>
        <button type="button" onClick={copy} aria-label={copied ? "代码块已复制" : "复制代码块"}>
          {copied ? <Check size={13} /> : <Copy size={13} />}{copied ? "已复制" : "复制"}
        </button>
      </div>
      <pre>{children}</pre>
      {failed && <div className="markdown-copy-error" role="status">复制失败，请选中代码手动复制。</div>}
    </div>
  );
}

const components: Components = {
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  a: ({ node: _node, href, children, ...props }) => (
    <a {...props} href={href} target={href?.startsWith("#") ? undefined : "_blank"}
      rel="noopener noreferrer">{children}</a>
  ),
  table: ({ node: _node, ...props }) => <div className="markdown-table"><table {...props} /></div>,
  img: ({ node: _node, ...props }) => <img {...props} loading="lazy" referrerPolicy="no-referrer" />,
};
const plugins = [remarkGfm];

// Keep raw HTML disabled and retain react-markdown's default safe URL transform.
// Memoization prevents reparsing unchanged messages on each Agent status poll.
export default memo(function MarkdownMessage({ text }: { text: string }) {
  return <div className="markdown-body"><ReactMarkdown skipHtml remarkPlugins={plugins}
    components={components}>{text}</ReactMarkdown></div>;
});
