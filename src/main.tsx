import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { SettingsWindow } from "./components/settings/SettingsWindow";
import { FullScreenSkeleton } from "./components/chat/MessageSkeleton";
import "./styles/globals.css";

// App 懒加载：独立设置窗口（index.html#settings）只渲染 SettingsWindow，
// 不会因此加载聊天主 bundle（react-markdown / katex / 语法高亮 / mermaid 等），
// 设置窗口打开速度大幅提升。主窗口加载 App 时显示骨架屏。
const App = lazy(() => import("./App"));
const isSettingsWindow = window.location.hash.replace(/^#/, "") === "settings";

if (import.meta.env.VITE_RIPPLE_E2E === "true") {
  void import("@wdio/tauri-plugin");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isSettingsWindow ? (
      <SettingsWindow />
    ) : (
      <Suspense fallback={<FullScreenSkeleton />}>
        <App />
      </Suspense>
    )}
  </React.StrictMode>,
);
