import { useEffect, useState } from "react";
import { systemService } from "@/services/system.service";

/**
 * IPC 健康检测：组件挂载时 ping 后端，返回连接状态。
 * null = 检测中，true = 正常，false = 异常。
 */
export function useIpcStatus() {
  const [ipcOk, setIpcOk] = useState<boolean | null>(null);

  useEffect(() => {
    systemService
      .ping()
      .then(() => setIpcOk(true))
      .catch(() => setIpcOk(false));
  }, []);

  return ipcOk;
}
