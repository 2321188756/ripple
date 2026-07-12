import type { Theme, ThemeDefinition } from "@/types/theme";
import { themeService } from "@/services/theme.service";

export const ACTIVE_THEME_KEY = "ripple-active-theme-id";
const CUSTOM_VARS_KEY = "ripple-custom-vars";

export function getActiveThemeId() {
  return localStorage.getItem(ACTIVE_THEME_KEY) || "default-light";
}

function getEffectiveMode(theme: Theme) {
  return theme === "system"
    ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    : theme;
}

function clearWallpaper() {
  document.body.style.backgroundImage = "";
  document.body.style.backgroundSize = "";
  document.body.style.backgroundPosition = "";
  document.body.style.backgroundAttachment = "";
  document.body.classList.remove("has-wallpaper");
}

function clearCustomVariables() {
  try {
    const keys: string[] = JSON.parse(localStorage.getItem(CUSTOM_VARS_KEY) || "[]");
    keys.forEach((key) => document.documentElement.style.removeProperty(key));
  } catch {
    // A malformed local preference should not prevent the base theme from loading.
  }
}

export async function applyThemeDefinition(themeDefinition: ThemeDefinition, theme: Theme, persist = true) {
  clearCustomVariables();
  clearWallpaper();

  const palette = themeDefinition.colors[getEffectiveMode(theme)] || themeDefinition.colors.light;
  const keys = Object.keys(palette);
  keys.forEach((key) => document.documentElement.style.setProperty(key, palette[key]));
  localStorage.setItem(CUSTOM_VARS_KEY, JSON.stringify(keys));

  if (persist) localStorage.setItem(ACTIVE_THEME_KEY, themeDefinition.id);

  if (themeDefinition.wallpaper) {
    try {
      const dataUrl = await themeService.readWallpaperBase64(themeDefinition.wallpaper);
      document.body.style.backgroundImage = `url(${dataUrl})`;
      document.body.style.backgroundSize = "cover";
      document.body.style.backgroundPosition = "center";
      document.body.style.backgroundAttachment = "fixed";
      document.body.classList.add("has-wallpaper");
    } catch (error) {
      console.error("wallpaper load error:", error);
    }
  }
}

export function clearThemeDefinition() {
  clearCustomVariables();
  clearWallpaper();
}
