import { Skeleton } from "@/components/ui/skeleton";

/** 消息加载骨架屏：模拟几条消息气泡的占位，用于 Suspense fallback / 初始加载。 */
export function MessageSkeleton({ count = 3 }: { count?: number }) {
  return (
    <div className="flex-1 overflow-hidden p-4 space-y-6">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className={`flex gap-2.5 ${i % 2 === 0 ? "justify-start" : "justify-end"}`}>
          {i % 2 === 0 && <Skeleton className="h-7 w-7 rounded-full shrink-0" />}
          <div className={`space-y-2 ${i % 2 === 0 ? "" : "items-end"}`}>
            <Skeleton className="h-4 w-64" />
            <Skeleton className="h-4 w-48" />
            {i % 2 === 0 && <Skeleton className="h-4 w-32" />}
          </div>
          {i % 2 !== 0 && <Skeleton className="h-7 w-7 rounded-full shrink-0" />}
        </div>
      ))}
    </div>
  );
}

/** 全屏居中加载骨架（App Suspense fallback） */
export function FullScreenSkeleton() {
  return (
    <div className="flex h-screen items-center justify-center bg-background">
      <div className="flex flex-col items-center gap-4">
        <div className="w-12 h-12 rounded-2xl bg-gradient-to-br from-primary to-primary-600 animate-pulse" />
        <Skeleton className="h-4 w-32" />
      </div>
    </div>
  );
}
