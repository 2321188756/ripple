import { memo, useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { registerLanguages } from "./languages";
import type { Components } from "react-markdown";
import "katex/dist/katex.min.css";

registerLanguages();

let mermaidInitialized = false;

function MermaidBlock({ code }: { code: string }) {
  const [svg, setSvg] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const m = (await import("mermaid")).default;
        if (!mermaidInitialized) {
          m.initialize({ startOnLoad: false, theme: document.documentElement.classList.contains("dark") ? "dark" : "default", securityLevel: "loose" });
          mermaidInitialized = true;
        }
        const id = `mermaid-${Math.random().toString(36).slice(2, 8)}`;
        const { svg } = await m.render(id, code);
        if (!cancelled) setSvg(svg);
      } catch { if (!cancelled) setFailed(true); }
    })();
    return () => { cancelled = true; };
  }, [code]);
  if (failed) return <pre className="bg-destructive/10 text-destructive p-2 rounded text-xs">Mermaid render failed</pre>;
  if (svg) return <div dangerouslySetInnerHTML={{ __html: svg }} />;
  return <pre className="text-xs">{code}</pre>;
}

const components: Components = {
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    const code = String(children).replace(/\n$/, "");
    if (match) {
      if (match[1] === "mermaid") return <MermaidBlock code={code} />;
      return <SyntaxHighlighter style={oneDark} language={match[1]} PreTag="div" customStyle={{ fontSize: "0.8rem", borderRadius: "0.5rem", margin: "0.5rem 0" }}>{code}</SyntaxHighlighter>;
    }
    return <code className={className} {...props}>{children}</code>;
  },
  pre({ children }) { return <>{children}</>; },
  table({ children }) { return <div className="overflow-x-auto my-2"><table className="border-collapse border border-border text-xs">{children}</table></div>; },
  th({ children }) { return <th className="border border-border px-3 py-1.5 bg-muted font-semibold">{children}</th>; },
  td({ children }) { return <td className="border border-border px-3 py-1.5">{children}</td>; },
  a({ href, children }) { return <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline">{children}</a>; },
};

/** 补全 HTML 片段为完整文档（VCP wrapIncompleteHtml 模式） */
function wrapHtml(html: string): string {
  if (/<html/i.test(html)) return html;
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0;padding:0;font-family:system-ui,sans-serif;background:transparent;}</style><script>window.onerror=function(m){document.body.innerHTML='<div style=color:red;padding:16px;font-size:13px;>❌ '+m+'</div>'}</script></head><body>${html}</body></html>`;
}

/** iframe 高度自适应：取 documentElement.scrollHeight + 子元素溢出检测 */
function useIframeHeight(iframeRef: React.RefObject<HTMLIFrameElement | null>, docKey: string) {
  const [height, setHeight] = useState(500);
  useEffect(() => {
    const f = iframeRef.current;
    if (!f) return;
    const win = f.contentWindow;
    if (!win) return;
    let timer: ReturnType<typeof setTimeout>;
    const measure = () => {
      try {
        const doc = f.contentDocument || win.document;
        if (!doc?.body) return;
        // scrollHeight 是最可靠的全文高度
        let h = doc.documentElement.scrollHeight || doc.body.scrollHeight || 500;
        // 检查有无溢出元素（absolute/fixed）
        for (const el of doc.body.children) {
          const rect = el.getBoundingClientRect();
          const bottom = rect.bottom + 20;
          if (bottom > h) h = bottom;
        }
        setHeight(Math.max(Math.ceil(h), 500));
      } catch {}
    };
    // 加载后延迟测量（等待 JS 渲染、字体加载）
    measure();
    const t1 = setTimeout(measure, 300);
    const t2 = setTimeout(measure, 1000);
    try {
      const doc = f.contentDocument || win.document;
      if (doc) {
        const ro = new ResizeObserver(() => { clearTimeout(timer); timer = setTimeout(measure, 150); });
        ro.observe(doc.documentElement);
        ro.observe(doc.body);
        return () => { ro.disconnect(); clearTimeout(timer); clearTimeout(t1); clearTimeout(t2); };
      }
    } catch {}
  }, [docKey]);
  return height;
}

/** 用 iframe 渲染完整 HTML（含 CDN/vendor 脚本），sandbox 隔离 */
/** HTML 全页渲染：宽度直接取聊天区 <main> 的宽度，不受气泡 max-w 限制 */
function HtmlPreview({ html }: { html: string }) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const fullDoc = wrapHtml(html);
  const height = useIframeHeight(iframeRef, fullDoc);
  const [chatW, setChatW] = useState(800);

  useEffect(() => {
    const main = document.querySelector('main[role="main"]');
    if (!main) return;
    const ro = new ResizeObserver(() => setChatW(main.clientWidth));
    ro.observe(main);
    return () => ro.disconnect();
  }, []);

  return <div style={{ minHeight: height }}>
    <iframe ref={iframeRef} srcDoc={fullDoc}
      style={{ width: chatW, height: `${height}px`, border: "none", borderRadius: "8px", background: "transparent", display: "block" }}
      title="渲染页" sandbox="allow-scripts" />
  </div>;
}

/** 从 AI 回复中提取 HTML 部分 */
function extractHtml(raw: string): string {
  const start = raw.search(/<(!DOCTYPE html|html\b|head|script[\s>]|div[\s>]|canvas[\s>]|svg)/i);
  if (start === -1) return raw;
  let end = raw.length;
  for (const tag of ["</html>", "</script>", "</svg>", "</body>"]) {
    const pos = raw.lastIndexOf(tag);
    if (pos > start) { end = pos + tag.length; break; }
  }
  return raw.slice(start, end).trim();
}

function MarkdownRendererImpl({ content }: { content: string }) {
  const [showSource, setShowSource] = useState(false);
  const [copied, setCopied] = useState(false);

  let rawHtml = content.trim();
  if ((rawHtml.startsWith("'") && rawHtml.endsWith("'")) ||
      (rawHtml.startsWith("\"") && rawHtml.endsWith("\""))) rawHtml = rawHtml.slice(1, -1).trim();

  // 检测是否需要 iframe 渲染（含 HTML 页面、CDN/本地 vendor 脚本、canvas）
  const needsIframe = /<html\b/i.test(rawHtml) || /<!DOCTYPE html/i.test(rawHtml) || /\bsrc=["'](\/vendor\/|https?:\/\/)/i.test(rawHtml) || /<canvas[\s>]/i.test(rawHtml);
  const htmlSource = needsIframe ? extractHtml(rawHtml) : rawHtml;

  const doCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text).then(() => { setCopied(true); setTimeout(() => setCopied(false), 2000); }).catch(() => {});
  }, []);

  if (showSource) {
    return <div className="relative">
      <pre className="bg-muted border border-border rounded-lg p-3 text-xs overflow-auto max-h-96 whitespace-pre-wrap font-mono">{htmlSource}</pre>
      <div className="absolute top-1.5 right-1.5 flex gap-1.5">
        <button onClick={() => doCopy(htmlSource)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm min-w-[36px] text-center">{copied ? "✓" : "复制"}</button>
        <button onClick={() => setShowSource(false)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm">渲染视图</button>
      </div>
    </div>;
  }

  // 完整 HTML 页面 / vendor 脚本 / canvas → iframe 渲染
  if (needsIframe) {
    return <div className="relative group/block my-2">
      <HtmlPreview html={htmlSource} />
      <div className="absolute top-2 right-2 flex gap-1.5 opacity-0 group-hover/block:opacity-100 transition-opacity">
        <button onClick={() => doCopy(rawHtml)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm min-w-[36px] text-center">{copied ? "✓" : "复制"}</button>
        <button onClick={() => setShowSource(true)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm">原文</button>
      </div>
    </div>;
  }

  // 普通 Markdown / 简单 HTML
  return <div className="relative group/block my-2">
    <div className="prose prose-sm dark:prose-invert max-w-none [&_p]:my-0.5 [&_ul]:my-0.5 [&_ol]:my-0.5 [&_li]:my-0 [&_pre]:my-1 [&_h1]:my-1 [&_h2]:my-1 [&_h3]:my-1 [&_h4]:my-1 [&_blockquote]:my-1 [&_table]:my-1">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeRaw, rehypeKatex]} components={components}>{rawHtml}</ReactMarkdown>
    </div>
    {(rawHtml.startsWith("<") || rawHtml.includes("<div") || rawHtml.includes("<style")) && (
      <div className="absolute top-2 right-2 flex gap-1.5 opacity-0 group-hover/block:opacity-100 transition-opacity">
        <button onClick={() => doCopy(rawHtml)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm min-w-[36px] text-center">{copied ? "✓" : "复制"}</button>
        <button onClick={() => setShowSource(true)} className="bg-background/80 backdrop-blur-sm border border-border rounded-md px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground shadow-sm">原文</button>
      </div>
    )}
  </div>;
}

export default memo(MarkdownRendererImpl);
