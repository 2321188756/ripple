import { create } from "zustand";
import type { SettingsTab } from "@/types/theme";

interface UIState {
  /** 设置面板是否打开 */
  settingsOpen: boolean;
  /** 设置面板当前 tab */
  settingsTab: SettingsTab;
  /** 侧边栏是否展开（桌面端持久偏好） */
  sidebarOpen: boolean;
  /** 窄窗口侧边栏抽屉是否打开（临时状态，不影响桌面端偏好） */
  mobileSidebarOpen: boolean;

  setSettingsOpen: (open: boolean) => void;
  toggleSettings: () => void;
  setSettingsTab: (tab: SettingsTab) => void;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  setMobileSidebarOpen: (open: boolean) => void;
  toggleMobileSidebar: () => void;
}

export const useUIStore = create<UIState>((set) => ({
  settingsOpen: false,
  settingsTab: "settings",
  sidebarOpen: true,
  mobileSidebarOpen: false,

  setSettingsOpen: (open) => set({ settingsOpen: open }),
  toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
  setSettingsTab: (tab) => set({ settingsTab: tab }),
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  setSidebarOpen: (open) => set({ sidebarOpen: open }),
  setMobileSidebarOpen: (open) => set({ mobileSidebarOpen: open }),
  toggleMobileSidebar: () => set((s) => ({ mobileSidebarOpen: !s.mobileSidebarOpen })),
}));
