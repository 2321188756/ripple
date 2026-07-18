import path from "node:path";
import type { Options } from "@wdio/types";

const appBinary = process.env.RIPPLE_E2E_BINARY
  ? path.resolve(process.env.RIPPLE_E2E_BINARY)
  : path.resolve("src-tauri/target/release/ripple-app.exe");

export const config: Options.Testrunner = {
  runner: "local",
  specs: ["./e2e/specs/**/*.spec.ts"],
  maxInstances: 1,
  services: [
    [
      "tauri",
      {
        appBinaryPath: appBinary,
        driverProvider: "external",
        autoInstallTauriDriver: true,
        autoDownloadEdgeDriver: true,
        captureBackendLogs: false,
        captureFrontendLogs: false,
        startTimeout: 120_000,
        commandTimeout: 30_000,
      },
    ],
  ],
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: appBinary,
      },
    },
  ],
  logLevel: "info",
  bail: 1,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 90_000,
  },
};
