#!/usr/bin/env python3
"""clipboard plugin: 读写系统剪贴板。多工具，靠 RIPPLE_TOOL 分派。
Windows 用 PowerShell Get/Set-Clipboard（可靠，避免 tkinter 的 X11 display 问题）；
其他平台用 tkinter。
"""
import json, os, sys, subprocess, platform


def read_clipboard():
    if platform.system() == "Windows":
        r = subprocess.run(
            ["powershell", "-NoProfile", "-Command",
             "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; Get-Clipboard -Raw"],
            capture_output=True, text=True, encoding="utf-8", timeout=5,
        )
        return r.stdout.rstrip("\r\n")
    else:
        import tkinter as tk
        root = tk.Tk()
        root.withdraw()
        try:
            return root.clipboard_get()
        finally:
            root.destroy()


def write_clipboard(text):
    if platform.system() == "Windows":
        # 通过 stdin 传文本，避免引号/特殊字符转义问题
        subprocess.run(
            ["powershell", "-NoProfile", "-Command",
             "[Console]::InputEncoding=[System.Text.Encoding]::UTF8; Set-Clipboard -Value ([Console]::In.ReadToEnd())"],
            input=text, text=True, encoding="utf-8", capture_output=True, timeout=5,
        )
        return f"已写入剪贴板 ({len(text)} 字符)"
    else:
        import tkinter as tk
        root = tk.Tk()
        root.withdraw()
        root.clipboard_clear()
        root.clipboard_append(text)
        root.update()
        root.destroy()
        return f"已写入剪贴板 ({len(text)} 字符)"


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    tool = os.environ.get("RIPPLE_TOOL", "")
    try:
        if tool == "read_clipboard":
            result = read_clipboard()
        elif tool == "write_clipboard":
            result = write_clipboard(args.get("text", ""))
        else:
            print(json.dumps({"error": f"unknown tool: {tool!r}"}, ensure_ascii=False))
            return
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
