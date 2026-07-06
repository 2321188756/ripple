#!/usr/bin/env python3
"""code-runner plugin: 执行 python/node 代码并返回输出。
代码写到临时文件后用对应解释器执行（避免 -c 的引号转义问题），有超时和输出截断保护。
参数通过 stdin 传入（JSON + 换行）。
"""
import json, os, sys, subprocess, tempfile

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))

LANGUAGES = {
    "python": {"cmd_key": "python_cmd", "default_cmd": "python", "suffix": ".py"},
    "node": {"cmd_key": "node_cmd", "default_cmd": "node", "suffix": ".js"},
}


def load_config():
    cfg_path = os.path.join(PLUGIN_DIR, "config.json")
    try:
        with open(cfg_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def main():
    raw = sys.stdin.readline().strip()
    try:
        args = json.loads(raw) if raw else {}
    except Exception as e:
        print(json.dumps({"error": f"parse args: {e}"}, ensure_ascii=False))
        return

    language = args.get("language", "").strip()
    code = args.get("code", "")
    if not code.strip():
        print(json.dumps({"error": "missing code"}, ensure_ascii=False))
        return

    lang = LANGUAGES.get(language)
    if not lang:
        print(json.dumps({"error": f"unsupported language: {language!r} (use python or node)"}, ensure_ascii=False))
        return

    cfg = load_config()
    interpreter = cfg.get(lang["cmd_key"], lang["default_cmd"])
    timeout = int(args.get("timeout", 15))
    max_output = int(cfg.get("max_output", 10000))

    # 写临时文件执行
    tmp = tempfile.NamedTemporaryFile(mode="w", suffix=lang["suffix"], delete=False, encoding="utf-8")
    tmp.write(code)
    tmp.close()
    try:
        try:
            result = subprocess.run(
                [interpreter, tmp.name],
                capture_output=True, text=True,
                timeout=timeout, encoding="utf-8", errors="replace",
            )
        except subprocess.TimeoutExpired:
            print(json.dumps({"error": f"timeout after {timeout}s (process killed)"}, ensure_ascii=False))
            return
        except FileNotFoundError:
            print(json.dumps({"error": f"interpreter not found: {interpreter}（{language} 未安装或不在 PATH?）"}, ensure_ascii=False))
            return
        except Exception as e:
            print(json.dumps({"error": f"exec failed: {e}"}, ensure_ascii=False))
            return

        stdout = result.stdout or ""
        stderr = result.stderr or ""
        if len(stdout) > max_output:
            stdout = stdout[:max_output] + "\n[...truncated]"
        if len(stderr) > max_output:
            stderr = stderr[:max_output] + "\n[...truncated]"

        text = f"[{language} exit {result.returncode}]\nstdout:\n{stdout}".rstrip()
        if stderr:
            text += f"\nstderr:\n{stderr}"
        print(json.dumps({"result": text}, ensure_ascii=False))
    finally:
        try:
            os.unlink(tmp.name)
        except Exception:
            pass


if __name__ == "__main__":
    main()
