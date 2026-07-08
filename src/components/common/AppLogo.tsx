import { cn } from "@/lib/utils";

interface AppLogoProps {
  /** sm = 侧边栏（20px），md = 头部（24px） */
  size?: "sm" | "md";
  className?: string;
}

/** Ripple Logo：圆角方块 + R 字母。统一品牌标识，消除多处重复渲染。 */
export function AppLogo({ size = "md", className }: AppLogoProps) {
  const box = size === "sm" ? "w-5 h-5 rounded-md" : "w-6 h-6 rounded-lg";
  const letter = size === "sm" ? "text-[9px]" : "text-[10px]";
  return (
    <div
      className={cn("bg-primary flex items-center justify-center shadow-sm", box, className)}
      aria-hidden="true"
    >
      <span className={cn("text-primary-foreground font-bold", letter)}>R</span>
    </div>
  );
}
