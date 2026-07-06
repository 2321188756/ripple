import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  build: {
    // Tauri 跑在系统 webview（Win=WebView2 / macOS=WKWebView），可直接用最新语法，跳过降级
    target: "esnext",
    chunkSizeWarningLimit: 900,
    rollupOptions: {
      output: {
        // 拆分大依赖为独立 chunk，便于并行加载与缓存命中，首屏不再解析单个 1.5MB 主包
        manualChunks: {
          "radix-vendor": [
            "@radix-ui/react-dialog",
            "@radix-ui/react-dropdown-menu",
            "@radix-ui/react-popover",
            "@radix-ui/react-select",
            "@radix-ui/react-tooltip",
            "@radix-ui/react-scroll-area",
            "@radix-ui/react-tabs",
          ],
          markdown: ["react-markdown", "remark-gfm", "remark-math", "rehype-katex"],
          "syntax-highlight": ["react-syntax-highlighter"],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
