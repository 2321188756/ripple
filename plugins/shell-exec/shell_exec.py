#!/usr/bin/env python3
"""shell-exec plugin: 执行 shell 命令并返回输出。
参数通过 stdin 传入（JSON + 换行）。优先用 bash（跨平台一致），不可用则回退系统默认 shell。
"""
import json, os, sys, subprocess, shutil

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))
# bash 若在 PATH 中则优先用（Windows 装了 Git Bash 即可用，命令体感一致）
BASH = shutil.which("bash")


def load_config():
    cfg_path = os.path.join(PLUGIN_DIR, "config.json")
    try:
        with open(cfg_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def _decode_bytes(data: bytes) -> str:
    """优先 UTF-8 解码，失败回退 GBK（Windows 中文系统常见子进程输出编码）。"""
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return data.decode("gbk", errors="replace")


def main():
    raw = sys.stdin.readline().strip()
    try:
        args = json.loads(raw) if raw else {}
    except Exception as e:
        print(json.dumps({"error": f"parse args: {e}"}, ensure_ascii=False))
        return

    command = args.get("command", "").strip()
    if not command:
        print(json.dumps({"error": "missing command"}, ensure_ascii=False))
        return

    cfg = load_config()
    cwd = args.get("cwd", "").strip() or cfg.get("default_cwd", "").strip() or os.path.expanduser("~")
    if not os.path.isdir(cwd):
        print(json.dumps({"error": f"cwd not a directory: {cwd}"}, ensure_ascii=False))
        return
    timeout = int(args.get("timeout", 30))
    max_output = int(cfg.get("max_output", 10000))

    try:
        if BASH:
            # 取原始字节，智能解码：优先 UTF-8，失败回退 GBK（Windows 中文系统默认）
            result = subprocess.run(
                [BASH, "-c", command], cwd=cwd,
                capture_output=True, text=False, timeout=timeout,
            )
            stdout = _decode_bytes(result.stdout)
            stderr = _decode_bytes(result.stderr)
        else:
            result = subprocess.run(
                command, shell=True, cwd=cwd,
                capture_output=True, text=True,
                timeout=timeout, encoding="utf-8", errors="replace",
            )
            stdout = result.stdout or ""
            stderr = result.stderr or ""
    except subprocess.TimeoutExpired:
        print(json.dumps({"error": f"timeout after {timeout}s (command killed)"}, ensure_ascii=False))
        return
    except Exception as e:
        print(json.dumps({"error": f"exec failed: {e}"}, ensure_ascii=False))
        return

    if len(stdout) > max_output:
        stdout = stdout[:max_output] + "\n[...truncated]"
    if len(stderr) > max_output:
        stderr = stderr[:max_output] + "\n[...truncated]"

    text = f"[exit {result.returncode}]\nstdout:\n{stdout}".rstrip()
    if stderr:
        text += f"\nstderr:\n{stderr}"
    print(json.dumps({"result": text}, ensure_ascii=False))


if __name__ == "__main__":
    main()
