import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/**
 * 打开独立的设置窗口。若已存在则聚焦，避免重复开。
 * 设置窗口加载 index.html#settings，前端按 hash 路由渲染 SettingsWindow。
 * 独立 OS 窗口自带原生拖动/缩放/置顶，比应用内浮层好用。
 */
export async function openSettingsWindow() {
  // 先尝试聚焦已存在的设置窗口（权限不足则忽略，回退到创建）
  try {
    const existing = await WebviewWindow.getByLabel("settings");
    if (existing) {
      await existing.setFocus().catch(() => {});
      return;
    }
  } catch {
    /* getByLabel 失败不阻断，继续尝试创建 */
  }

  try {
    const url = new URL(window.location.href);
    url.hash = "settings";
    new WebviewWindow("settings", {
      url: url.href,
      title: "设置 — Ripple",
      width: 900,
      height: 700,
      minWidth: 560,
      minHeight: 380,
    });
  } catch (e) {
    console.error("[openSettings] failed to create window:", e);
  }
}
