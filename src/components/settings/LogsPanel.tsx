import { useMemo, useState } from "react";
import { RefreshCw, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useLogs } from "@/hooks/useLogs";
import { cn } from "@/lib/utils";
import type { LogEntry, LogLevel } from "@/services/log.service";

interface LogsPanelProps {
  active: boolean;
}

type LevelFilter = "all" | Exclude<LogLevel, "unknown"> | "unknown";

const LEVEL_STYLES: Record<LogLevel, string> = {
  trace: "text-muted-foreground",
  debug: "text-muted-foreground",
  info: "text-foreground",
  warn: "text-warning",
  error: "text-destructive",
  unknown: "text-muted-foreground",
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function matchesText(entry: LogEntry, query: string): boolean {
  if (!query) return true;
  return `${entry.timestamp ?? ""} ${entry.level} ${entry.target ?? ""} ${entry.message}`
    .toLocaleLowerCase()
    .includes(query);
}

/** 日志面板：有界轮询快照、级别/文本过滤和显式跟随控制。 */
export function LogsPanel({ active }: LogsPanelProps) {
  const {
    snapshot,
    error,
    loading,
    follow,
    setFollowing,
    refresh,
    scrollRef,
    handleScroll,
  } = useLogs(active);
  const [level, setLevel] = useState<LevelFilter>("all");
  const [search, setSearch] = useState("");

  const normalizedSearch = search.trim().toLocaleLowerCase();
  const visibleEntries = useMemo(
    () => snapshot.entries.filter((entry) =>
      (level === "all" || entry.level === level) && matchesText(entry, normalizedSearch)),
    [snapshot.entries, level, normalizedSearch],
  );

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
          <Input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="筛选日志文本"
            aria-label="筛选日志文本"
            className="pl-8"
          />
        </div>
        <Select value={level} onValueChange={(value) => setLevel(value as LevelFilter)}>
          <SelectTrigger className="w-32" aria-label="日志级别">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部级别</SelectItem>
            <SelectItem value="trace">TRACE</SelectItem>
            <SelectItem value="debug">DEBUG</SelectItem>
            <SelectItem value="info">INFO</SelectItem>
            <SelectItem value="warn">WARN</SelectItem>
            <SelectItem value="error">ERROR</SelectItem>
            <SelectItem value="unknown">未解析</SelectItem>
          </SelectContent>
        </Select>
        <label className="flex h-9 items-center gap-2 rounded-md border border-input bg-background px-3 text-xs text-muted-foreground">
          <Switch checked={follow} onCheckedChange={setFollowing} aria-label="跟随最新日志" />
          跟随最新
        </label>
        <Button variant="outline" size="sm" onClick={() => void refresh()} disabled={loading}>
          <RefreshCw className={cn("mr-1.5 h-3.5 w-3.5", loading && "animate-spin")} aria-hidden="true" />
          刷新
        </Button>
      </div>

      {error && (
        <div role="alert" className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          无法读取日志：{error}
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 text-xs text-muted-foreground">
        <span>显示 {visibleEntries.length} / {snapshot.returnedLines} 行</span>
        <span className="min-w-0 truncate" title={snapshot.path || undefined}>
          {snapshot.path ? `${formatBytes(snapshot.fileSize)} · ${snapshot.path}` : "等待日志快照"}
        </span>
      </div>

      {snapshot.truncated && (
        <div className="rounded-md border border-border bg-muted/50 px-3 py-2 text-xs text-muted-foreground">
          当前仅显示文件尾部快照（最多 {snapshot.requestedLines} 行 / {formatBytes(snapshot.byteCap)}）。
        </div>
      )}

      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="min-h-0 flex-1 overflow-auto rounded-lg border border-border bg-card p-3 font-mono text-[11px] leading-relaxed"
        aria-label="日志内容"
      >
        {visibleEntries.length === 0 ? (
          <div className="text-muted-foreground">{snapshot.entries.length === 0 ? "暂无日志" : "没有匹配的日志"}</div>
        ) : visibleEntries.map((entry, index) => (
          <div
            key={`${entry.timestamp ?? "unknown"}-${index}-${entry.raw}`}
            className="grid grid-cols-[minmax(11rem,auto)_4rem_minmax(0,1fr)] gap-x-2 border-b border-border/40 py-0.5 last:border-b-0"
          >
            <span className="text-muted-foreground">{entry.timestamp ?? "—"}</span>
            <span className={cn("font-semibold uppercase", LEVEL_STYLES[entry.level])}>{entry.level}</span>
            <span className="min-w-0 whitespace-pre-wrap break-words text-foreground">
              {entry.target && <span className="text-muted-foreground">{entry.target}: </span>}
              {entry.message}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
