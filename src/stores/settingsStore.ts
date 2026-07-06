import { create } from "zustand";
import { getSetting, setSetting } from "@/services/settings.service";

interface SettingsState {
  apiKey: string;
  apiBaseUrl: string;
  defaultModel: string;
  llmModel: string;
  loaded: boolean;

  load: () => Promise<void>;
  setApiKey: (v: string) => Promise<void>;
  setApiBaseUrl: (v: string) => Promise<void>;
  setDefaultModel: (v: string) => Promise<void>;
  setLlmModel: (v: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  apiKey: "",
  apiBaseUrl: "http://192.168.0.123:3000/v1",
  defaultModel: "deepseek-v4-flash",
  llmModel: "deepseek-v4-flash",
  loaded: false,

  load: async () => {
    const [apiKey, apiBaseUrl, defaultModel, llmModel] = await Promise.all([
      getSetting("api_key"),
      getSetting("api_base_url"),
      getSetting("default_model"),
      getSetting("llm_model"),
    ]);
    set({
      apiKey: apiKey || "",
      apiBaseUrl: apiBaseUrl || "http://192.168.0.123:3000/v1",
      defaultModel: defaultModel || "deepseek-v4-flash",
      llmModel: llmModel || "deepseek-v4-flash",
      loaded: true,
    });
  },

  setApiKey: async (v: string) => {
    await setSetting("api_key", v);
    set({ apiKey: v });
  },
  setApiBaseUrl: async (v: string) => {
    await setSetting("api_base_url", v);
    set({ apiBaseUrl: v });
  },
  setDefaultModel: async (v: string) => {
    await setSetting("default_model", v);
    set({ defaultModel: v });
  },
  setLlmModel: async (v: string) => {
    await setSetting("llm_model", v);
    set({ llmModel: v });
  },
}));
