#!/usr/bin/env python3
"""reminder plugin: 桌面提醒弹窗。延迟由独立进程处理，不阻塞。
Windows 用 ctypes.windll.user32.MessageBoxW（独立进程内，弹窗是模态但不阻塞插件返回）。
"""
import json, os, sys, subprocess


def remind(message, delay=0, title="Ripple 提醒"):
    if not message:
        raise ValueError("missing message")
    if sys.platform != "win32":
        raise RuntimeError("reminder plugin currently Windows-only")
    # 独立进程脚本：sleep(delay) 后弹 MessageBox。message/title 用 repr 安全转成 python 字面量。
    inner = (
        "import time, ctypes\n"
        f"time.sleep({int(delay)})\n"
        f"ctypes.windll.user32.MessageBoxW(0, {message!r}, {title!r}, 0x40)\n"
    )
    # DETACHED_PROCESS (0x8)：独立进程，不阻塞插件
    subprocess.Popen([sys.executable, "-c", inner], creationflags=0x00000008)
    if delay > 0:
        return f"已设定提醒：{delay}s 后弹出「{title}」"
    return f"已弹出提醒「{title}」"


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = remind(
            args.get("message", ""),
            int(args.get("delay_seconds", 0)),
            args.get("title", "Ripple 提醒"),
        )
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
