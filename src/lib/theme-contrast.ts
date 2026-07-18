import type { ThemeDefinition } from "@/types/theme";

export type ThemeValidationWarning = {
  mode: "light" | "dark";
  token: string;
  kind: "invalid-hsl" | "low-contrast";
  message: string;
  ratio?: number;
};

export type ParsedHsl = { h: number; s: number; l: number };

/** Parses the canonical raw CSS theme token format: "H S% L%". */
export function parseHslToken(value: string): { ok: true; value: ParsedHsl } | { ok: false; reason: string } {
  const match = /^\s*([-+]?(?:\d+(?:\.\d+)?|\.\d+))\s+([-+]?(?:\d+(?:\.\d+)?|\.\d+))%\s+([-+]?(?:\d+(?:\.\d+)?|\.\d+))%\s*$/.exec(value);
  if (!match) return { ok: false, reason: "必须为 H S% L% 格式" };
  const [h, s, l] = match.slice(1).map(Number);
  if (![h, s, l].every(Number.isFinite)) return { ok: false, reason: "包含无效数字" };
  if (s < 0 || s > 100 || l < 0 || l > 100) return { ok: false, reason: "饱和度和亮度必须介于 0% 到 100%" };
  return { ok: true, value: { h: ((h % 360) + 360) % 360, s, l } };
}

function hslToLinearRgb({ h, s, l }: ParsedHsl): [number, number, number] {
  s /= 100;
  l /= 100;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) [r, g, b] = [c, x, 0];
  else if (h < 120) [r, g, b] = [x, c, 0];
  else if (h < 180) [r, g, b] = [0, c, x];
  else if (h < 240) [r, g, b] = [0, x, c];
  else if (h < 300) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const linearize = (channel: number) => {
    const value = channel + m;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return [linearize(r), linearize(g), linearize(b)];
}

function relativeLuminance(value: string): number | null {
  const parsed = parseHslToken(value);
  if (!parsed.ok) return null;
  const [r, g, b] = hslToLinearRgb(parsed.value);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio (1-21) for valid HSL tokens; invalid values return 0. */
export function contrastRatio(fg: string, bg: string): number {
  const first = relativeLuminance(fg);
  const second = relativeLuminance(bg);
  if (first === null || second === null) return 0;
  const [hi, lo] = first > second ? [first, second] : [second, first];
  return (hi + 0.05) / (lo + 0.05);
}

/** Adjusts a valid foreground token until it reaches the target contrast. */
export function ensureContrast(fg: string, bg: string, target = 4.5): string {
  const foreground = parseHslToken(fg);
  const backgroundLuminance = relativeLuminance(bg);
  if (!foreground.ok || backgroundLuminance === null || contrastRatio(fg, bg) >= target) return fg;
  const { h, s, l } = foreground.value;
  const backgroundIsDark = backgroundLuminance < 0.5;
  let newLightness = l;
  for (let i = 0; i < 50; i++) {
    newLightness = Math.max(0, Math.min(100, newLightness + (backgroundIsDark ? 2 : -2)));
    const candidate = `${h} ${s}% ${newLightness}%`;
    if (contrastRatio(candidate, bg) >= target) return candidate;
  }
  return `${h} ${s}% ${backgroundIsDark ? 100 : 0}%`;
}

const SEMANTIC_PAIRS: Array<[string, string]> = [
  ["--foreground", "--background"],
  ["--card-foreground", "--card"],
  ["--popover-foreground", "--popover"],
  ["--primary-foreground", "--primary"],
  ["--secondary-foreground", "--secondary"],
  ["--muted-foreground", "--muted"],
  ["--accent-foreground", "--accent"],
  ["--destructive-foreground", "--destructive"],
  ["--warning-foreground", "--warning"],
  ["--success-foreground", "--success"],
  ["--info-foreground", "--info"],
  ["--sidebar-foreground", "--sidebar-background"],
  ["--sidebar-primary-foreground", "--sidebar-primary"],
  ["--sidebar-accent-foreground", "--sidebar-accent"],
];

/**
 * Validates supplied HSL tokens and corrects low-contrast semantic foregrounds.
 * Optional pairs are only checked when both tokens are present.
 */
export function validateThemeContrast(theme: ThemeDefinition): { theme: ThemeDefinition; warnings: ThemeValidationWarning[] } {
  const warnings: ThemeValidationWarning[] = [];
  const fixed: ThemeDefinition = {
    ...theme,
    colors: { light: { ...theme.colors.light }, dark: { ...theme.colors.dark } },
  };

  for (const mode of ["light", "dark"] as const) {
    const palette = fixed.colors[mode];
    for (const [token, value] of Object.entries(palette)) {
      const parsed = parseHslToken(value);
      if (!parsed.ok) {
        warnings.push({
          mode,
          token,
          kind: "invalid-hsl",
          message: `${mode === "light" ? "浅色" : "深色"}模式 ${token} 的颜色值无效：${parsed.reason}`,
        });
      }
    }
    for (const [foregroundToken, backgroundToken] of SEMANTIC_PAIRS) {
      const foreground = palette[foregroundToken];
      const background = palette[backgroundToken];
      if (!foreground || !background || !parseHslToken(foreground).ok || !parseHslToken(background).ok) continue;
      const ratio = contrastRatio(foreground, background);
      if (ratio < 4.5) {
        palette[foregroundToken] = ensureContrast(foreground, background);
        warnings.push({
          mode,
          token: `${foregroundToken} / ${backgroundToken}`,
          kind: "low-contrast",
          ratio,
          message: `${mode === "light" ? "浅色" : "深色"}模式 ${foregroundToken} 对比度 ${ratio.toFixed(2)} < 4.5，已自动调整`,
        });
      }
    }
  }
  return { theme: fixed, warnings };
}
