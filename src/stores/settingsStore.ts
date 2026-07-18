import { create } from "zustand";
import { getSetting, setSetting, settingsService } from "@/services/settings.service";

interface SettingsState {
  hasApiKey: boolean;
  apiBaseUrl: string;
  defaultModel: string;
  llmModel: string;
  loaded: boolean;

  load: () => Promise<void>;
  saveApiKey: (value: string) => Promise<void>;
  clearApiKey: () => Promise<void>;
  setApiBaseUrl: (value: string) => Promise<void>;
  setDefaultModel: (value: string) => Promise<void>;
  setLlmModel: (value: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  hasApiKey: false,
  apiBaseUrl: "http://192.168.0.123:3000/v1",
  defaultModel: "deepseek-v4-flash",
  llmModel: "deepseek-v4-flash",
  loaded: false,

  load: async () => {
    const [hasApiKey, apiBaseUrl, defaultModel, llmModel] = await Promise.all([
      settingsService.hasApiKey(),
      getSetting("api_base_url"),
      getSetting("default_model"),
      getSetting("llm_model"),
    ]);
    set({
      hasApiKey,
      apiBaseUrl: apiBaseUrl || "http://192.168.0.123:3000/v1",
      defaultModel: defaultModel || "deepseek-v4-flash",
      llmModel: llmModel || "deepseek-v4-flash",
      loaded: true,
    });
  },

  saveApiKey: async (value: string) => {
    await settingsService.saveApiKey(value);
    set({ hasApiKey: true });
  },
  clearApiKey: async () => {
    await settingsService.clearApiKey();
    set({ hasApiKey: false });
  },
  setApiBaseUrl: async (value: string) => {
    await setSetting("api_base_url", value);
    set({ apiBaseUrl: value });
  },
  setDefaultModel: async (value: string) => {
    await setSetting("default_model", value);
    set({ defaultModel: value });
  },
  setLlmModel: async (value: string) => {
    await setSetting("llm_model", value);
    set({ llmModel: value });
  },
}));
