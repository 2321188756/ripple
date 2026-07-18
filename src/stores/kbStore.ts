import { create } from "zustand";
import { kbService } from "@/services/kb.service";
import { useSettingsStore } from "./settingsStore";
import type { KnowledgeBase, Document } from "@/types";

interface KBState {
  kbs: KnowledgeBase[];
  docs: Record<string, Document[]>;
  loading: boolean;
  loadKBs: () => Promise<void>;
  loadDocs: (kbId: string) => Promise<void>;
  createKB: (name: string, desc?: string) => Promise<string>;
  deleteKB: (id: string) => Promise<void>;
  importDoc: (kbId: string, filePath: string) => Promise<void>;
}

export const useKBStore = create<KBState>((set, get) => ({
  kbs: [],
  docs: {},
  loading: false,

  loadKBs: async () => {
    const kbs = await kbService.listKBs().catch(() => []);
    set({ kbs });
  },

  loadDocs: async (kbId: string) => {
    const docs = await kbService.listDocs(kbId).catch(() => []);
    set((s) => ({ docs: { ...s.docs, [kbId]: docs } }));
  },

  createKB: async (name: string, desc?: string) => {
    const kb = await kbService.createKB(name, desc);
    await get().loadKBs();
    return kb.id;
  },

  deleteKB: async (id: string) => {
    await kbService.deleteKB(id);
    await get().loadKBs();
  },

  importDoc: async (kbId: string, filePath: string) => {
    const s = useSettingsStore.getState();
    await kbService.importDoc({
      kbId,
      filePath,
      apiBaseUrl: s.apiBaseUrl,
    });
    await get().loadDocs(kbId);
  },
}));
