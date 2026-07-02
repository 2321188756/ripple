import { Skeleton } from "@/components/ui/skeleton";

/** 消息加载骨架屏 */
export function MessageSkeleton() {
  return (
    <div className="px-4 py-2 space-y-2">
      {[0, 1, 2].map((i) => (
        <div key={i} className="flex flex-col gap-1.5">
          <Skeleton className="h-3 w-1/3" />
          <Skeleton className="h-3 w-2/3" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      ))}
    </div>
  );
}
