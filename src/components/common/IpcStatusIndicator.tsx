import { cn } from "@/lib/utils";

interface IpcStatusIndicatorProps {
  status: boolean | null;
  className?: string;
}

/** IPC 连接状态指示灯：黄=检测中，绿=正常，红=异常 */
export function IpcStatusIndicator({ status, className }: IpcStatusIndicatorProps) {
  return (
    <span
      role="status"
      aria-label={
        status === null
          ? "正在检测连接"
          : status
            ? "已连接"
            : "连接异常"
      }
      className={cn(
        "inline-block w-1.5 h-1.5 rounded-full animate-pulse-dot",
        status === null
          ? "bg-yellow-400"
          : status
            ? "bg-green-500"
            : "bg-red-500",
        className,
      )}
    />
  );
}
