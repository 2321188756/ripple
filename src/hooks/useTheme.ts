import { useCallback, useEffect, useState } from "react";
import type { Theme, ThemeDefinition } from "@/types/theme";
import { applyThemeDefinition, clearThemeDefinition, getActiveThemeId } from "@/lib/theme-runtime";

const STORAGE_KEY = "ripple-theme";

function applyMode(theme: Theme) {
  const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", isDark);
}

/** 主题切换 hook（light / dark / system + 自定义主题）。 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === "undefined") return "system";
    return (localStorage.getItem(STORAGE_KEY) as Theme) || "system";
  });
  const [activeThemeId, setActiveThemeId] = useState<string>(() => getActiveThemeId());

  useEffect(() => {
    applyMode(theme);
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => applyMode("system");
    query.addEventListener("change", handleChange);
    return () => query.removeEventListener("change", handleChange);
  }, [theme]);

  const setTheme = (nextTheme: Theme) => setThemeState(nextTheme);

  const applyCustomTheme = useCallback(async (themeDefinition: ThemeDefinition) => {
    await applyThemeDefinition(themeDefinition, theme);
    setActiveThemeId(themeDefinition.id);
  }, [theme]);

  const previewTheme = useCallback(async (themeDefinition: ThemeDefinition) => {
    await applyThemeDefinition(themeDefinition, theme, false);
  }, [theme]);

  const revertPreview = useCallback(() => {
    clearThemeDefinition();
    setActiveThemeId(getActiveThemeId());
  }, []);

  const isDark = theme === "dark" || (theme === "system" && typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches);

  return { theme, setTheme, isDark, activeThemeId, applyCustomTheme, previewTheme, revertPreview };
}
