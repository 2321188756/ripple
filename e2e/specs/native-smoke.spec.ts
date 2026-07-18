describe("Ripple native smoke", () => {
  const invoke = async <T>(command: string, args: Record<string, unknown> = {}): Promise<T> =>
    browser.executeAsync((cmd: string, invokeArgs: Record<string, unknown>, done: (value: unknown) => void) => {
      window.__TAURI__.core.invoke(cmd, invokeArgs).then(
        (value: unknown) => done({ ok: true, value }),
        (error: unknown) => done({ ok: false, error: String(error) }),
      );
    }, command, args).then((result) => {
      const response = result as { ok: boolean; value?: T; error?: string };
      if (!response.ok) throw new Error(response.error ?? `invoke ${command} failed`);
      return response.value as T;
    });

  it("boots the native app and serves core IPC", async () => {
    const title = await browser.getTitle();
    expect(title).toContain("Ripple");

    const pong = await invoke<string>("ping", { message: "native-e2e" });
    expect(pong).toBe("pong: native-e2e");

    const knowledgeBases = await invoke<unknown[]>("list_kbs");
    expect(knowledgeBases).toEqual([]);
  });

  it("opens the settings window and renders real log entries", async () => {
    const settingsButtons = await browser.$$('button[aria-label="设置"]');
    const settingsButton = settingsButtons[0] ?? (await browser.$("button=全局设置"));
    await settingsButton.waitForExist();
    await browser.execute((element: HTMLElement) => element.click(), settingsButton);

    await browser.waitUntil(async () => (await browser.getWindowHandles()).length === 2);
    const handles = await browser.getWindowHandles();
    await browser.switchToWindow(handles[1]);

    const heading = await browser.$("h1");
    await heading.waitForDisplayed();
    expect(await heading.getText()).toBe("通用设置");

    const logsTab = await browser.$('button[aria-current], nav button:last-child');
    await browser.execute((element: HTMLElement) => {
      const buttons = Array.from(document.querySelectorAll("nav button"));
      const target = buttons.find((button) => button.textContent?.includes("日志"));
      (target ?? element).click();
    }, logsTab);
    await browser.waitUntil(
      async () => (await browser.$("body").getText()).includes("Ripple starting"),
      { timeout: 15_000, timeoutMsg: "logs panel did not render the native startup log" },
    );

    const snapshot = await invoke<{
      returnedLines: number;
      entries: Array<{ raw: string }>;
    }>("get_logs", { lines: 100 });
    expect(snapshot.returnedLines).toBeGreaterThan(0);
    expect(snapshot.entries.some((entry) => entry.raw.includes("Ripple starting"))).toBe(true);

    await browser.closeWindow();
  });
});
