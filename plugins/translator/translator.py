#!/usr/bin/env python3
"""translator plugin: 文本翻译。用 MyMemory 免费 API（无需 key，日限 5000 词）。stdlib urllib。"""
import json, sys, html, urllib.request, urllib.parse


def _detect_lang(text):
    # 简单检测：含 CJK 字符 → zh，否则 en
    for c in text:
        if "一" <= c <= "鿿":
            return "zh"
    return "en"


def translate(text, target="zh", source=""):
    if not text.strip():
        raise ValueError("missing text")
    target = target or "zh"
    source = source or _detect_lang(text)
    if source == target:
        return {"translated": text, "source": source, "target": target, "note": "源语言与目标相同，未翻译"}
    # MyMemory 限 500 字符
    q = text[:500]
    url = "https://api.mymemory.translated.net/get?" + urllib.parse.urlencode({
        "q": q, "langpair": f"{source}|{target}",
    })
    req = urllib.request.Request(url, headers={"User-Agent": "Ripple/1.0"})
    resp = urllib.request.urlopen(req, timeout=10)
    data = json.loads(resp.read().decode("utf-8", errors="replace"))
    status = data.get("responseStatus", 0)
    translated = data.get("responseData", {}).get("translatedText", "")
    if not translated or status not in (200, "200"):
        raise RuntimeError(f"translate API error (status={status}): {data.get('responseDetails', '')}")
    # MyMemory 可能返回 HTML 实体
    translated = html.unescape(translated)
    return {"translated": translated, "source": source, "target": target}


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = translate(
            args.get("text", ""),
            args.get("target", "zh"),
            args.get("source", ""),
        )
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
