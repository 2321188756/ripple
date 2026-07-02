import { create } from "zustand";
import type { SettingsTab } from "@/types/theme";

interface UIState {
  /** 设置面板是否打开 */
  settingsOpen: boolean;
  /** 设置面板当前 tab */
  settingsTab: SettingsTab;
  /** 侧边栏是否展开（响应式/可折叠） */
  sidebarOpen: boolean;

  setSettingsOpen: (open: boolean) => void;
  toggleSettings: () => void;
  setSettingsTab: (tab: SettingsTab) => void;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
}

export const useUIStore = create<UIState>((set) => ({
  settingsOpen: false,
  settingsTab: "settings",
  sidebarOpen: true,

  setSettingsOpen: (open) => set({ settingsOpen: open }),
  toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
  setSettingsTab: (tab) => set({ settingsTab: tab }),
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  setSidebarOpen: (open) => set({ sidebarOpen: open }),
}));
