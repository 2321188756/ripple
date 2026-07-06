import { useCallback, useEffect, useState } from "react";
import type { Theme } from "@/types/theme";
import type { ThemeDefinition } from "@/types/theme";

const STORAGE_KEY = "ripple-theme";
const THEME_ID_KEY = "ripple-active-theme-id";
const CUSTOM_VARS_KEY = "ripple-custom-vars";

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  root.classList.toggle("dark", isDark);
}

/** 将主题的 CSS 变量应用到 :root / .dark */
function applyThemeVars(themeDef: ThemeDefinition | null) {
  const root = document.documentElement;
  // 清除之前应用的自定义变量
  const prev = localStorage.getItem(CUSTOM_VARS_KEY);
  if (prev) {
    try { JSON.parse(prev).forEach((key: string) => root.style.removeProperty(key)); } catch {}
  }
  if (!themeDef) return;
  const vars: string[] = [];
  for (const [key, val] of Object.entries(themeDef.colors.light || {})) {
    root.style.setProperty(key, val);
    vars.push(key);
  }
  for (const [key, val] of Object.entries(themeDef.colors.dark || {})) {
    root.style.setProperty(key, val);
    vars.push(key);
  }
  localStorage.setItem(CUSTOM_VARS_KEY, JSON.stringify(vars));
}

/**
 * 主题切换 hook（light / dark / system + 自定义主题）。
 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === "undefined") return "system";
    return (localStorage.getItem(STORAGE_KEY) as Theme) || "system";
  });
  const [activeThemeId, setActiveThemeId] = useState<string>(
    () => localStorage.getItem(THEME_ID_KEY) || "default-light"
  );

  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, [theme]);

  const setTheme = (t: Theme) => setThemeState(t);

  /** 应用自定义主题（CSS 变量覆盖） */
  const applyCustomTheme = useCallback((themeDef: ThemeDefinition) => {
    applyThemeVars(themeDef);
    setActiveThemeId(themeDef.id);
    localStorage.setItem(THEME_ID_KEY, themeDef.id);
  }, []);

  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);

  return { theme, setTheme, isDark, activeThemeId, applyCustomTheme };
}
