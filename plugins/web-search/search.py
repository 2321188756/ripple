#!/usr/bin/env python3
"""web-search plugin: 搜索网络并返回结果"""
import json, urllib.request, urllib.parse, html

def search_duckduckgo(query: str, max_results: int = 5) -> list:
    """使用 DuckDuckGo HTML 搜索"""
    url = f"https://html.duckduckgo.com/html/?q={urllib.parse.quote(query)}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        html_content = resp.read().decode("utf-8", errors="replace")
    except Exception as e:
        return [{"title": "Error", "url": "", "snippet": f"Search failed: {e}"}]

    results = []
    for line in html_content.split("\n"):
        if 'class="result__a"' in line and len(results) < max_results:
            # Extract title
            title_start = line.find('>', line.find('class="result__a"')) + 1
            title_end = line.find("</a>", title_start)
            title = html.unescape(line[title_start:title_end].strip()) if title_start > 0 and title_end > 0 else ""

            # Extract URL from the <a> tag
            url_start = line.find('href="')
            if url_start > 0:
                url_start += 6
                url_end = line.find('"', url_start)
                url = html.unescape(line[url_start:url_end]) if url_end > 0 else ""
            else:
                url = ""

            if title:
                results.append({"title": title, "url": url, "snippet": ""})

    return results[:max_results]


def main():
    try:
        raw = input()
        args = json.loads(raw)
        query = args.get("query", "")
        max_results = int(args.get("max_results", 5))

        if not query:
            print(json.dumps({"error": "Missing query"}))
            return

        results = search_duckduckgo(query, max_results)
        output = f"搜索「{query}」的结果：\n\n"
        for i, r in enumerate(results, 1):
            output += f"{i}. {r['title']}\n   {r['url']}\n\n"

        print(json.dumps({"result": output.strip()}))
    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    main()
