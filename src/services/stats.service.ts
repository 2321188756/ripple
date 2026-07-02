import { invoke } from "./invoke";
import type { UsageStats } from "@/types";

export const statsService = {
  getUsage: (): Promise<UsageStats> =>
    invoke<UsageStats>("get_usage_stats"),
};
