#!/usr/bin/env python3
"""screenshot plugin: 截屏保存到文件，返回路径。Windows 用 PowerShell .NET Bitmap。"""
import json, os, sys, subprocess, time


def capture(save_dir=None):
    save_dir = save_dir or os.path.join(os.path.expanduser("~"), "ripple_screenshots")
    os.makedirs(save_dir, exist_ok=True)
    ts = time.strftime("%Y%m%d-%H%M%S")
    path = os.path.join(save_dir, f"screenshot-{ts}.png")
    # PS 单引号字符串：内部单引号转义为 ''
    ps_path = path.replace("'", "''")
    ps = (
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing;"
        "$b = [System.Windows.Forms.SystemInformation]::VirtualScreen;"
        "$bmp = New-Object System.Drawing.Bitmap($b.Width, $b.Height);"
        "$g = [System.Drawing.Graphics]::FromImage($bmp);"
        "$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size);"
        f"$bmp.Save('{ps_path}');"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, timeout=15)
    if r.returncode != 0 or not os.path.exists(path):
        raise RuntimeError(f"capture failed: {r.stderr.strip() or 'unknown'}")
    return path


def main():
    raw = sys.stdin.readline().strip()
    args = json.loads(raw) if raw else {}
    try:
        path = capture(args.get("save_dir", ""))
        print(json.dumps({"result": f"截屏已保存: {path}"}, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
