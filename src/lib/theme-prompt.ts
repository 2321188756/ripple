import { getSetting, setSetting } from "@/services/settings.service";

/** 默认 AI 主题生成 prompt 模板。__KW__ 会被替换为用户需求描述。 */
export const DEFAULT_THEME_PROMPT = `你是资深 UI 视觉设计师，擅长为桌面应用创作有氛围感、有美感的主题配色。遵循 60-30-10 色彩法则与色系统一原则。

用户需求：「__KW__」

创作 3 套风格各异、有设计感的主题。核心原则：

【背景必须有色彩，禁止纯黑/纯白】
- 深色主题背景：带色相的深色，L 10-18%，S 15-35%。例：深靛蓝 "240 30% 13%"、深酒红 "350 28% 12%"、深森林 "155 22% 11%"、深墨青 "190 32% 12%"。绝不要 "0 0% 0%" 或 L<8% 的死黑。
- 浅色主题背景：带暖/冷调的灰白，L 93-97%，S 5-18%。例：暖米白 "40 16% 97%"、冷雾灰 "220 12% 96%"。绝不要 "0 0% 100%" 纯白。

【主色是灵魂】primary 饱和度 S 55-78%，是视觉锚点，与背景色相形成对比/互补/邻近关系营造氛围。

【色系统一，拒绝纯灰】card/muted/border/sidebar-background 都微染主色相（同 H 调 S/L）。例：主色靛蓝(240)时 card 用 "240 16% 16%"(深) 或 "240 10% 99%"(浅)，而非 "0 0% 15%" 纯灰。

【双色创作，3 套风格各异】不同色相/明暗/氛围。根据需求情绪定调：「赛博朋克」=霓虹青紫高饱和；「温暖拿铁」=暖棕米白柔光；「雨后森林」=墨绿青灰湿润；「星空夜幕」=深靛紫罗兰微光；「极简留白」=低饱和灰白克制。

【对比度】深色背景 foreground L≥88%；浅色背景 foreground L≤22%。对比度≥4.5:1。

返回纯 JSON 数组（无 markdown、无解释），每个元素：
{"id":"ai-x","name":"2-4字中文名","description":"氛围一句话","isBuiltin":false,"colors":{"light":{"--background":"H S% L%","--foreground":"H S% L%","--primary":"H S% L%","--card":"H S% L%","--border":"H S% L%","--muted":"H S% L%","--sidebar-background":"H S% L%"},"dark":{同样7个键}},"agentStyle":{"icon_color":"#hex","border_color":"#hex","border_width":2,"name_color":"#hex"}}

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
