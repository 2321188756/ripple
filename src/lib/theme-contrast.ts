import type { ThemeDefinition } from "@/types/theme";

/** 解析 "H S% L%" 字符串为 [h, s, l] 数值 */
function parseHsl(str: string): [number, number, number] {
  const parts = str.trim().split(/\s+/);
  const h = Number(parts[0]) || 0;
  const s = Number(parts[1]?.replace("%", "")) || 0;
  const l = Number(parts[2]?.replace("%", "")) || 0;
  return [h, s, l];
}

/** HSL -> 线性 RGB（0-1） */
function hslToLinearRgb(h: number, s: number, l: number): [number, number, number] {
  s /= 100; l /= 100;
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
  const lin = (v: number) => (v + m <= 0.03928 ? (v + m) / 12.92 : ((v + m + 0.055) / 1.055) ** 2.4);
  return [lin(r), lin(g), lin(b)];
}

/** 相对亮度（WCAG） */
function relativeLuminance(hslStr: string): number {
  const [h, s, l] = parseHsl(hslStr);
  const [r, g, b] = hslToLinearRgb(h, s, l);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** 对比度比值（1-21），≥4.5 达 WCAG AA */
export function contrastRatio(fg: string, bg: string): number {
  const l1 = relativeLuminance(fg);
  const l2 = relativeLuminance(bg);
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

/** 调整前景色 L 值直到与背景对比度 ≥ target（默认 4.5）。返回新的 "H S% L%" */
export function ensureContrast(fg: string, bg: string, target = 4.5): string {
  if (contrastRatio(fg, bg) >= target) return fg;
  const [h, s, l] = parseHsl(fg);
  const bgDark = relativeLuminance(bg) < 0.5;
  let newL = l;
  for (let i = 0; i < 50; i++) {
    newL = bgDark ? newL + 2 : newL - 2;
    newL = Math.max(0, Math.min(100, newL));
    const candidate = `${h} ${s}% ${newL}%`;
    if (contrastRatio(candidate, bg) >= target) return candidate;
  }
  return `${h} ${s}% ${bgDark ? 95 : 5}%`; // 兜底：纯白/纯黑
}

/**
 * 校验主题前景/背景对比度，不达标自动调整前景色。
 * 返回修正后的主题 + 警告列表（供 UI 提示）。
 */
export function validateThemeContrast(theme: ThemeDefinition): {
  theme: ThemeDefinition;
  warnings: string[];
} {
  const warnings: string[] = [];
  const fixed: ThemeDefinition = {
    ...theme,
    colors: {
      light: { ...theme.colors.light },
      dark: { ...theme.colors.dark },
    },
  };
  for (const mode of ["light", "dark"] as const) {
    const palette = fixed.colors[mode];
    const bg = palette["--background"];
    const fg = palette["--foreground"];
    if (bg && fg) {
      const ratio = contrastRatio(fg, bg);
      if (ratio < 4.5) {
        palette["--foreground"] = ensureContrast(fg, bg);
        warnings.push(`${mode === "light" ? "浅色" : "深色"}模式前景对比度 ${ratio.toFixed(2)} < 4.5，已自动调整`);
      }
    }
  }
  return { theme: fixed, warnings };
}
