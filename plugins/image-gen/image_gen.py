#!/usr/bin/env python3
"""image-gen plugin: 文生图。调 OpenAI 兼容 /images/generations，保存 PNG 返回路径。
凭证 RIPPLE_API_KEY/RIPPLE_API_BASE 由后端 exec_process 注入（AI 工具调用链）。
"""
import json, os, sys, time, base64, urllib.request


PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))


def load_config():
    try:
        with open(os.path.join(PLUGIN_DIR, "config.json"), "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def generate_image(prompt, size="1024x1024", model=""):
    if not prompt.strip():
        raise ValueError("missing prompt")
    api_key = os.environ.get("RIPPLE_API_KEY", "")
    api_base = os.environ.get("RIPPLE_API_BASE", "").rstrip("/")
    if not api_key or not api_base:
        raise RuntimeError("缺少 RIPPLE_API_KEY/RIPPLE_API_BASE（AI 工具调用时后端注入；手动 execute_plugin_tool 不传凭证）")
    cfg = load_config()
    model = model or cfg.get("image_model", "dall-e-3")

    url = f"{api_base}/images/generations"
    body = json.dumps({"model": model, "prompt": prompt, "n": 1, "size": size})
    req = urllib.request.Request(url, data=body.encode("utf-8"), headers={
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    })
    resp = urllib.request.urlopen(req, timeout=120)
    data = json.loads(resp.read().decode("utf-8", errors="replace"))
    items = data.get("data") or []
    if not items:
        raise RuntimeError(f"API 未返回图片数据: {data}")
    item = items[0]

    save_dir = os.path.join(os.path.expanduser("~"), "ripple_images")
    os.makedirs(save_dir, exist_ok=True)
    path = os.path.join(save_dir, f"image-{time.strftime('%Y%m%d-%H%M%S')}.png")
    if item.get("b64_json"):
        with open(path, "wb") as f:
            f.write(base64.b64decode(item["b64_json"]))
    elif item.get("url"):
        urllib.request.urlretrieve(item["url"], path)
    else:
        raise RuntimeError(f"响应无 url/b64_json: {item}")
    return f"图片已生成: {path}"


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = generate_image(
            args.get("prompt", ""),
            args.get("size", "1024x1024"),
            args.get("model", ""),
        )
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
