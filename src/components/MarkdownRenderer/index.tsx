import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { Components } from "react-markdown";
import "katex/dist/katex.min.css";

let mermaidInitialized = false;

const components: Components = {
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    const code = String(children).replace(/\n$/, "");
    if (match) {
      if (match[1] === "mermaid") {
        return <div className="mermaid-wrapper" data-mermaid={code}>{code}</div>;
      }
      return (
        <SyntaxHighlighter
          style={oneDark}
          language={match[1]}
          PreTag="div"
          customStyle={{ fontSize: "0.8rem", borderRadius: "0.5rem", margin: "0.5rem 0" }}
        >
          {code}
        </SyntaxHighlighter>
      );
    }
    return <code className={className} {...props}>{children}</code>;
  },
  pre({ children }) { return <>{children}</>; },
  table({ children }) {
    return (
      <div className="overflow-x-auto my-2">
        <table className="border-collapse border border-border text-xs">{children}</table>
      </div>
    );
  },
  th({ children }) {
    return <th className="border border-border px-3 py-1.5 bg-muted font-semibold">{children}</th>;
  },
  td({ children }) {
    return <td className="border border-border px-3 py-1.5">{children}</td>;
  },
  a({ href, children }) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline">
        {children}
      </a>
    );
  },
};

export default function MarkdownRenderer({ content }: { content: string }) {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!rootRef.current) return;
    const wrappers = rootRef.current.querySelectorAll<HTMLDivElement>(".mermaid-wrapper");
    if (wrappers.length === 0) return;

    (async () => {
      try {
        const mermaid = (await import("mermaid")).default;
        if (!mermaidInitialized) {
          mermaid.initialize({
            startOnLoad: false,
            theme: document.documentElement.classList.contains("dark") ? "dark" : "default",
            securityLevel: "loose",
          });
          mermaidInitialized = true;
        }
        for (const el of wrappers) {
          const code = el.getAttribute("data-mermaid") || el.textContent || "";
          const id = `mermaid-${Math.random().toString(36).slice(2, 8)}`;
          try {
            const { svg } = await mermaid.render(id, code);
            el.innerHTML = svg;
            el.classList.remove("mermaid-wrapper");
          } catch {
            el.outerHTML = `<pre class="bg-destructive/10 text-destructive p-2 rounded text-xs">Mermaid render failed</pre>`;
          }
        }
      } catch { /* mermaid not loaded */ }
    })();
  }, [content]);

  return (
    <div className="prose prose-sm dark:prose-invert max-w-none [&_p]:my-0.5 [&_ul]:my-0.5 [&_ol]:my-0.5 [&_li]:my-0 [&_pre]:my-1 [&_h1]:my-1 [&_h2]:my-1 [&_h3]:my-1 [&_h4]:my-1 [&_blockquote]:my-1 [&_table]:my-1">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKatex]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
