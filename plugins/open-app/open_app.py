#!/usr/bin/env python3
"""open-app plugin: 用默认程序打开文件/URL/应用。跨平台。"""
import json, os, sys, subprocess, platform


def open_path(path):
    path = path.strip()
    if not path:
        raise ValueError("missing path")
    sys_name = platform.system()
    if sys_name == "Windows":
        os.startfile(path)
    elif sys_name == "Darwin":
        subprocess.run(["open", path], check=True)
    else:
        subprocess.run(["xdg-open", path], check=True)
    return f"已打开: {path}"


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = open_path(args.get("path", ""))
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
