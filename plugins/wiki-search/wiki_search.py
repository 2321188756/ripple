#!/usr/bin/env python3
"""wiki-search plugin: 搜索 Wikipedia，返回条目标题/URL/摘要。stdlib urllib，无依赖。
注意：部分地区 Wikipedia 不可达，需配置 HTTPS_PROXY 代理或 VPN。
"""
import json, sys, urllib.request, urllib.parse, urllib.error


def wiki_search(query, lang="zh", limit=3):
    if not query.strip():
        raise ValueError("missing query")
    lang = lang or "zh"
    base = f"https://{lang}.wikipedia.org/w/api.php"
    params = {
        "action": "query",
        "generator": "search",
        "gsrsearch": query,
        "gsrlimit": str(int(limit)),
        "prop": "extracts",
        "exintro": "1",
        "explaintext": "1",
        "format": "json",
        "formatversion": "2",
    }
    url = base + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"User-Agent": "Ripple/1.0"})
    try:
        resp = urllib.request.urlopen(req, timeout=12)
    except urllib.error.URLError as e:
        raise RuntimeError(f"Wikipedia 不可达（{e.reason if hasattr(e, 'reason') else e}）。该网络环境可能屏蔽 Wikipedia，需配置 HTTPS_PROXY 代理或 VPN。")
    data = json.loads(resp.read().decode("utf-8", errors="replace"))
    pages = data.get("query", {}).get("pages", []) or []
    results = []
    for p in pages:
        title = p.get("title", "")
        extract = (p.get("extract") or "").strip()
        page_url = f"https://{lang}.wikipedia.org/wiki/" + urllib.parse.quote(title.replace(" ", "_"))
        results.append({"title": title, "url": page_url, "summary": extract[:800]})
    if not results:
        return f"未找到「{query}」相关条目"
    return results


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = wiki_search(
            args.get("query", ""),
            args.get("lang", "zh"),
            int(args.get("limit", 3)),
        )
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
