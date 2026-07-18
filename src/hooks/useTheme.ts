import { useEffect, useSyncExternalStore } from "react";
import type { ThemeDefinition } from "@/types/theme";
import {
  activateTheme,
  getThemeSnapshot,
  initializeThemeRuntime,
  previewThemeDefinition,
  refreshActiveTheme,
  revertThemePreview,
  setThemeMode,
  subscribeTheme,
} from "@/lib/theme-runtime";

/** 主题切换 hook（light / dark / system + 自定义主题）。 */
export function useTheme() {
  const snapshot = useSyncExternalStore(subscribeTheme, getThemeSnapshot, getThemeSnapshot);

  useEffect(() => {
    void initializeThemeRuntime();
  }, []);

  return {
    theme: snapshot.theme,
    setTheme: setThemeMode,
    isDark: snapshot.effectiveMode === "dark",
    activeThemeId: snapshot.activeThemeId,
    initialized: snapshot.initialized,
    applyCustomTheme: (theme: ThemeDefinition) => activateTheme(theme),
    previewTheme: (theme: ThemeDefinition) => previewThemeDefinition(theme),
    revertPreview: revertThemePreview,
    refreshActiveTheme,
  };
}
