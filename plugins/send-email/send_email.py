#!/usr/bin/env python3
"""send-email plugin: 发送邮件。smtplib（stdlib）。SMTP 凭证存同目录 config.json。
端口 465 → SMTP_SSL；587/25 → SMTP + starttls。
"""
import json, os, sys, smtplib
from email.message import EmailMessage

PLUGIN_DIR = os.path.dirname(os.path.abspath(__file__))


def load_config():
    try:
        with open(os.path.join(PLUGIN_DIR, "config.json"), "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def send_email(to, subject, body):
    if not to.strip():
        raise ValueError("missing recipient (to)")
    cfg = load_config()
    host = cfg.get("smtp_host", "").strip()
    port = int(cfg.get("smtp_port", 465))
    user = cfg.get("smtp_user", "").strip()
    pwd = cfg.get("smtp_pass", "").strip()
    from_name = cfg.get("from_name", "Ripple") or "Ripple"
    if not (host and user and pwd):
        raise RuntimeError("SMTP 未配置，请在 plugins/send-email/config.json 填写 smtp_host/smtp_port/smtp_user/smtp_pass")

    msg = EmailMessage()
    msg["From"] = f"{from_name} <{user}>" if from_name else user
    msg["To"] = to
    msg["Subject"] = subject or "(no subject)"
    msg.set_content(body or "")

    if port == 465:
        with smtplib.SMTP_SSL(host, port, timeout=30) as s:
            s.login(user, pwd)
            s.send_message(msg)
    else:
        with smtplib.SMTP(host, port, timeout=30) as s:
            s.starttls()
            s.login(user, pwd)
            s.send_message(msg)
    return f"邮件已发送至 {to}"


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        result = send_email(
            args.get("to", ""),
            args.get("subject", ""),
            args.get("body", ""),
        )
        print(json.dumps({"result": result}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
