import { getSetting, setSetting } from "@/services/settings.service";

/** 默认 AI 主题生成 prompt 模板。__KW__ 会被替换为用户需求描述。 */
export const DEFAULT_THEME_PROMPT = `你是资深 UI 视觉设计师，擅长为桌面应用创作有氛围感、层次丰富且可读的主题配色。遵循 60-30-10 色彩法则与 WCAG 对比度要求。

用户需求：「__KW__」

创作 3 套风格各异、有设计感的完整主题。每套主题必须是一个真正多色的系统：
- 选择明确的 primary 交互色，以及与它有可见色相差、但和谐的 secondary/accent 色；根据气质采用互补、分裂互补、邻近或三角色相关系。
- gradient-from / gradient-via / gradient-to 必须使用 2-3 个协调但可辨别的色相，不能只是同一色相的明暗变化。
- warning、success、info、destructive 需要各自语义明确，同时保持整体氛围；不要把所有语义色都做成 primary 的同色相。
- 深色背景使用有色相的深色（L 10-18%，S 15-35%），浅色背景使用带暖/冷调的灰白（L 93-97%，S 5-18%）；禁止纯黑和纯白。
- 所有 surface（card、muted、sidebar 等）可轻微染色但要克制，让文字与控件层级清晰。
- 每个 foreground / background 配对对比度至少 4.5:1；深色 foreground L≥88%，浅色 foreground L≤22%。

返回纯 JSON 数组（无 markdown、无解释）。每个元素为：
{"id":"ai-x","name":"2-4字中文名","description":"氛围一句话","isBuiltin":false,"colors":{"light":PALETTE,"dark":PALETTE},"agentStyle":{"iconColor":"#hex","borderColor":"#hex","borderWidth":2,"nameColor":"#hex"}}

PALETTE 必须是一个对象，且 light 与 dark 都必须完整包含以下键，值均为 "H S% L%"：
--background,--foreground,--card,--card-foreground,--popover,--popover-foreground,--primary,--primary-foreground,--secondary,--secondary-foreground,--muted,--muted-foreground,--accent,--accent-foreground,--destructive,--destructive-foreground,--warning,--warning-foreground,--success,--success-foreground,--info,--info-foreground,--border,--input,--ring,--primary-50,--primary-100,--primary-200,--primary-300,--primary-400,--primary-500,--primary-600,--primary-700,--primary-800,--primary-900,--sidebar-background,--sidebar-foreground,--sidebar-primary,--sidebar-primary-foreground,--sidebar-accent,--sidebar-accent-foreground,--sidebar-border,--sidebar-ring,--gradient-from,--gradient-via,--gradient-to。

只返回 JSON 数组。`;

const THEME_PROMPT_KEY = "theme_prompt_template";

/** 加载 prompt 模板（用户自定义优先，否则默认） */
export async function loadThemePrompt(): Promise<string> {
  const saved = await getSetting(THEME_PROMPT_KEY);
  return saved || DEFAULT_THEME_PROMPT;
}

/** 保存 prompt 模板 */
export async function saveThemePrompt(template: string): Promise<void> {
  await setSetting(THEME_PROMPT_KEY, template);
}

/** 组装最终 prompt：模板替换 __KW__ */
export function buildPrompt(template: string, userInput: string): string {
  return template.replace(/__KW__/g, userInput);
}
