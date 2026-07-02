import { useStats } from "@/hooks/useStats";
import { Skeleton } from "@/components/ui/skeleton";
import { Card, CardContent } from "@/components/ui/card";

/** 用量统计面板 */
export function StatsPanel() {
  const { stats, loading } = useStats();

  if (loading || !stats) {
    return (
      <div className="space-y-3">
        <div className="grid grid-cols-3 gap-2">
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-16" />
          ))}
        </div>
        <Skeleton className="h-20" />
      </div>
    );
  }

  const maxDaily = Math.max(...stats.daily_stats.map((d) => d.messages), 1);

  return (
    <div className="space-y-4 text-xs">
      {/* 总览卡片 */}
      <div className="grid grid-cols-3 gap-2">
        <Card>
          <CardContent className="p-2 text-center">
            <div className="text-lg font-bold text-primary">{stats.total_conversations}</div>
            <div className="text-muted-foreground">对话</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-2 text-center">
            <div className="text-lg font-bold text-primary">{stats.total_messages}</div>
            <div className="text-muted-foreground">消息</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-2 text-center">
            <div className="text-lg font-bold text-primary">
              {(stats.total_tokens / 1000).toFixed(1)}K
            </div>
            <div className="text-muted-foreground">Tokens</div>
          </CardContent>
        </Card>
      </div>

      {/* 每日趋势 */}
      <div>
        <div className="font-medium text-muted-foreground mb-1">每日消息（近 30 天）</div>
        <div className="flex items-end gap-[2px] h-20">
          {stats.daily_stats
            .slice()
            .reverse()
            .map((d, i) => (
              <div
                key={i}
                className="flex-1 flex flex-col items-center justify-end"
                title={`${d.date}: ${d.messages} msgs, ${d.tokens} tokens`}
              >
                <div
                  style={{
                    height: `${(d.messages / maxDaily) * 100}%`,
                    minHeight: d.messages > 0 ? 4 : 0,
                  }}
                  className="w-full bg-primary/70 rounded-t"
                />
              </div>
            ))}
        </div>
        <div className="flex justify-between text-[9px] text-muted-foreground mt-1">
          <span>{stats.daily_stats[stats.daily_stats.length - 1]?.date?.slice(5) || ""}</span>
          <span>{stats.daily_stats[0]?.date?.slice(5) || ""}</span>
        </div>
      </div>

      {/* 角色分布 */}
      <div className="space-y-1">
        <div className="font-medium text-muted-foreground mb-1">按角色统计</div>
        {stats.messages_by_role.map((r) => (
          <div key={r.role} className="flex items-center gap-2">
            <span className="w-16 text-muted-foreground">{r.role}</span>
            <div className="flex-1 bg-muted rounded h-4 relative overflow-hidden">
              <div
                className="bg-primary h-full rounded"
                style={{ width: `${(r.count / stats.total_messages) * 100}%` }}
              />
            </div>
            <span className="w-12 text-right text-muted-foreground">{r.count}</span>
          </div>
        ))}
      </div>

      {/* 模型分布 */}
      {stats.top_models.length > 0 && (
        <div>
          <div className="font-medium text-muted-foreground mb-1">热门模型</div>
          {stats.top_models.map((m) => (
            <div key={m.model} className="flex justify-between text-xs py-0.5">
              <span className="text-foreground/80">{m.model}</span>
              <span className="text-muted-foreground">{m.conversations}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
