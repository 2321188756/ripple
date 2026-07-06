#!/usr/bin/env python3
"""url-fetch plugin: 获取网页内容并转为纯文本"""
import json, urllib.request, html, re

def fetch_url(url: str, max_chars: int = 5000) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        resp = urllib.request.urlopen(req, timeout=15)
        content = resp.read().decode("utf-8", errors="replace")
    except Exception as e:
        return f"Error fetching URL: {e}"

    # Strip HTML tags
    text = re.sub(r"<[^>]+>", " ", content)
    # Decode HTML entities
    text = html.unescape(text)
    # Collapse whitespace
    text = re.sub(r"\s+", " ", text).strip()
    # Remove common non-content artefacts
    text = re.sub(r"(?i)var\s+\w+|function\s*\(|\.addEventListener|document\.", "", text)

    if len(text) > max_chars:
        text = text[:max_chars] + "\n\n[Content truncated...]"

    return text


def main():
    try:
        raw = input()
        args = json.loads(raw)
        url = args.get("url", "")
        max_chars = int(args.get("max_chars", 5000))

        if not url:
            print(json.dumps({"error": "Missing url"}))
            return

        content = fetch_url(url, max_chars)
        output = f"Content from {url}:\n\n{content}"
        print(json.dumps({"result": output.strip()}))

    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    main()
