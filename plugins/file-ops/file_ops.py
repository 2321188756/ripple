#!/usr/bin/env python3
"""file-ops plugin: 本地文件读写/列表（限定允许目录）。
工具通过 RIPPLE_TOOL 环境变量分派（read_file / write_file / list_dir）。
参数通过 stdin 传入（JSON + 换行），结果以 JSON {"result":...} / {"error":...} 输出。
"""
import json, os, sys, fnmatch

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))


def load_config():
    """读取同目录 config.json"""
    cfg_path = os.path.join(PLUGIN_DIR, "config.json")
    try:
        with open(cfg_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def allowed_roots():
    """返回 canonicalize 后的允许根目录列表。留空默认用户主目录。"""
    cfg = load_config()
    roots = cfg.get("allowed_roots") or []
    if not roots:
        roots = [os.path.expanduser("~")]
    out = []
    for r in roots:
        try:
            out.append(os.path.realpath(r))
        except Exception:
            pass
    return out


def resolve_and_check(path):
    """解析路径（相对路径相对主目录）并检查是否在允许目录内，防 ../ 与符号链接逃逸。"""
    if not os.path.isabs(path):
        path = os.path.join(os.path.expanduser("~"), path)
    real = os.path.realpath(path)
    # 必须正好是某个 root，或在某个 root 之下（os.sep 防前缀歧义，如 /home vs /home2）
    if not any(real == r or real.startswith(r + os.sep) for r in allowed_roots()):
        raise PermissionError(f"path not in allowed roots: {real}")
    return real


def read_file(path, max_chars=10000):
    real = resolve_and_check(path)
    if not os.path.isfile(real):
        raise FileNotFoundError(f"not a file: {real}")
    with open(real, "r", encoding="utf-8", errors="replace") as f:
        content = f.read()
    if len(content) > max_chars:
        content = content[:max_chars] + "\n\n[...truncated]"
    return content


def write_file(path, content, append=False):
    real = resolve_and_check(path)
    parent = os.path.dirname(real)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(real, "a" if append else "w", encoding="utf-8") as f:
        f.write(content)
    return f"wrote {len(content)} chars to {real} ({'append' if append else 'overwrite'})"


def list_dir(path="", pattern=""):
    real = resolve_and_check(path)  # 空路径解析为主目录，仍经权限检查
    if not os.path.isdir(real):
        raise NotADirectoryError(f"not a directory: {real}")
    entries = []
    for name in sorted(os.listdir(real)):
        if pattern and not fnmatch.fnmatch(name, pattern):
            continue
        full = os.path.join(real, name)
        entries.append({
            "name": name,
            "type": "dir" if os.path.isdir(full) else "file",
            "size": os.path.getsize(full) if os.path.isfile(full) else 0,
        })
    return entries


def main():
    raw = sys.stdin.readline().strip()
    try:
        args = json.loads(raw) if raw else {}
    except Exception as e:
        print(json.dumps({"error": f"parse args: {e}"}, ensure_ascii=False))
        return

    tool = os.environ.get("RIPPLE_TOOL", "")
    try:
        if tool == "read_file":
            result = read_file(args.get("path", ""), int(args.get("max_chars", 10000)))
        elif tool == "write_file":
            result = write_file(args.get("path", ""), args.get("content", ""),
                                bool(args.get("append", False)))
        elif tool == "list_dir":
            result = list_dir(args.get("path", ""), args.get("pattern", ""))
        else:
            print(json.dumps({"error": f"unknown tool: {tool!r}"}, ensure_ascii=False))
            return
        print(json.dumps({"result": result}, ensure_ascii=False))
    except PermissionError as e:
        print(json.dumps({"error": f"permission denied: {e}"}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
