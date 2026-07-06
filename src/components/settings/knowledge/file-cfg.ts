import { FileText, FileCode } from "lucide-react";

/** 文件类型 → 图标/颜色/标签 配置，供 DocCard / DocPreviewDialog 复用。 */
export const FILE_CFG: Record<string, { icon: typeof FileText; color: string; label: string }> = {
  md:  { icon: FileText, color: "text-blue-500",  label: "MD" },
  txt: { icon: FileText, color: "text-slate-500", label: "TXT" },
  rs:  { icon: FileCode, color: "text-amber-500", label: "Rust" },
  py:  { icon: FileCode, color: "text-emerald-500", label: "Python" },
  js:  { icon: FileCode, color: "text-yellow-500", label: "JS" },
  ts:  { icon: FileCode, color: "text-cyan-500",   label: "TS" },
  pdf: { icon: FileText, color: "text-red-500",    label: "PDF" },
};

const DEFAULT_CFG = { icon: FileText, color: "text-muted-foreground", label: "?" };

export function FileCfg(ext: string) {
  return FILE_CFG[ext] || DEFAULT_CFG;
}
