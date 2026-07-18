import type { Theme, ThemeDefinition } from "@/types/theme";
import { themeService } from "@/services/theme.service";
import { parseHslToken } from "@/lib/theme-contrast";

export const ACTIVE_THEME_KEY = "ripple-active-theme-id";
const THEME_KEY = "ripple-theme";
const CUSTOM_VARS_KEY = "ripple-custom-vars";

type EffectiveMode = "light" | "dark";

export interface ThemeRuntimeSnapshot {
  theme: Theme;
  effectiveMode: EffectiveMode;
  activeThemeId: string;
  initialized: boolean;
}

const getStoredTheme = (): Theme => {
  if (typeof window === "undefined") return "system";
  const stored = localStorage.getItem(THEME_KEY);
  return stored === "light" || stored === "dark" || stored === "system" ? stored : "system";
};

const getEffectiveMode = (theme: Theme): EffectiveMode =>
  theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : theme === "dark" ? "dark" : "light";

let snapshot: ThemeRuntimeSnapshot = {
  theme: getStoredTheme(),
  effectiveMode: typeof window === "undefined" ? "light" : getEffectiveMode(getStoredTheme()),
  activeThemeId: typeof window === "undefined" ? "default-light" : getActiveThemeId(),
  initialized: false,
};
let activeTheme: ThemeDefinition | null = null;
let previewTheme: ThemeDefinition | null = null;
let initialization: Promise<void> | null = null;
let systemListenerInstalled = false;
let renderGeneration = 0;
const listeners = new Set<() => void>();

function publish(next: Partial<ThemeRuntimeSnapshot>) {
  snapshot = { ...snapshot, ...next };
  listeners.forEach((listener) => listener());
}

export function getActiveThemeId() {
  return typeof window === "undefined" ? "default-light" : localStorage.getItem(ACTIVE_THEME_KEY) || "default-light";
}

export function getThemeSnapshot(): ThemeRuntimeSnapshot {
  return snapshot;
}

export function subscribeTheme(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
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

function applyModeClass() {
  document.documentElement.classList.toggle("dark", snapshot.effectiveMode === "dark");
}

function applyDefinition(themeDefinition: ThemeDefinition | null) {
  const generation = ++renderGeneration;
  clearCustomVariables();
  clearWallpaper();

  if (!themeDefinition) {
    localStorage.removeItem(CUSTOM_VARS_KEY);
    return;
  }

  const palette = themeDefinition.colors[snapshot.effectiveMode] || themeDefinition.colors.light;
  const validEntries = Object.entries(palette).filter(([, value]) => parseHslToken(value).ok);
  validEntries.forEach(([key, value]) => document.documentElement.style.setProperty(key, value));
  localStorage.setItem(CUSTOM_VARS_KEY, JSON.stringify(validEntries.map(([key]) => key)));

  if (!themeDefinition.wallpaper) return;
  void themeService.readWallpaperBase64(themeDefinition.wallpaper)
    .then((dataUrl) => {
      if (generation !== renderGeneration) return;
      document.body.style.backgroundImage = `url(${dataUrl})`;
      document.body.style.backgroundSize = "cover";
      document.body.style.backgroundPosition = "center";
      document.body.style.backgroundAttachment = "fixed";
      document.body.classList.add("has-wallpaper");
    })
    .catch((error) => console.error("wallpaper load error:", error));
}

function renderCurrentTheme() {
  applyModeClass();
  applyDefinition(previewTheme || activeTheme);
}

function installSystemListener() {
  if (systemListenerInstalled || typeof window === "undefined") return;
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  query.addEventListener("change", () => {
    if (snapshot.theme !== "system") return;
    publish({ effectiveMode: getEffectiveMode("system") });
    renderCurrentTheme();
  });
  systemListenerInstalled = true;
}

/** Restores the persisted active theme for this webview context. */
export function initializeThemeRuntime(): Promise<void> {
  if (typeof window === "undefined") return Promise.resolve();
  if (snapshot.initialized) return Promise.resolve();
  if (initialization) return initialization;

  applyModeClass();
  installSystemListener();
  initialization = themeService.list()
    .then((themes) => {
      const persistedId = getActiveThemeId();
      const resolved = themes.find((theme) => theme.id === persistedId)
        || themes.find((theme) => theme.id === "default-light")
        || themes[0]
        || null;
      activeTheme = resolved;
      const activeThemeId = resolved?.id || "default-light";
      if (resolved && resolved.id !== persistedId) localStorage.setItem(ACTIVE_THEME_KEY, activeThemeId);
      publish({ activeThemeId, initialized: true });
      renderCurrentTheme();
    })
    .catch((error) => {
      console.error("theme restoration error:", error);
      publish({ initialized: true });
    })
    .finally(() => { initialization = null; });
  return initialization;
}

export async function setThemeMode(theme: Theme) {
  const normalized: Theme = theme === "light" || theme === "dark" || theme === "system" ? theme : "system";
  localStorage.setItem(THEME_KEY, normalized);
  publish({ theme: normalized, effectiveMode: getEffectiveMode(normalized) });
  renderCurrentTheme();
}

/** Commits a theme as the active persisted theme. */
export async function activateTheme(themeDefinition: ThemeDefinition) {
  activeTheme = themeDefinition;
  previewTheme = null;
  localStorage.setItem(ACTIVE_THEME_KEY, themeDefinition.id);
  publish({ activeThemeId: themeDefinition.id });
  renderCurrentTheme();
}

/** Renders a temporary theme without changing the persisted active selection. */
export async function previewThemeDefinition(themeDefinition: ThemeDefinition) {
  previewTheme = themeDefinition;
  renderCurrentTheme();
}

/** Restores the persisted active theme after a temporary preview. */
export async function revertThemePreview() {
  previewTheme = null;
  renderCurrentTheme();
}

/** Reloads the active definition after a theme editor or wallpaper update. */
export async function refreshActiveTheme() {
  const themes = await themeService.list();
  activeTheme = themes.find((theme) => theme.id === snapshot.activeThemeId)
    || themes.find((theme) => theme.id === "default-light")
    || null;
  renderCurrentTheme();
}

/** @deprecated Use activateTheme or previewThemeDefinition. */
export async function applyThemeDefinition(themeDefinition: ThemeDefinition, _theme: Theme, persist = true) {
  if (persist) await activateTheme(themeDefinition);
  else await previewThemeDefinition(themeDefinition);
}

/** @deprecated Use revertThemePreview. */
export function clearThemeDefinition() {
  previewTheme = null;
  renderCurrentTheme();
}
