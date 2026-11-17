#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把飞书应用凭证写入 TaskCap 的 feishu.json（方便命令行初始化）。

凭证只写进 %APPDATA%\\taskcap\\feishu.json，不进入仓库。
凭证来源：--app-id/--app-secret 参数，或环境变量 FEISHU_APP_ID/FEISHU_APP_SECRET，
或 `--from <json>` 指定的文件（形如 {"app_id": "...", "app_secret": "..."}）。
base token / table id 从命令行或环境变量取，也能沿用本机已有配置。

用法：
    python scripts/feishu/seed_config.py --app-token <token> --table-id <id> --enable
    python scripts/feishu/seed_config.py --from D:\\path\\feishu-cred.json --enable
"""
import argparse
import json
import os
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")


def target_path() -> Path:
    appdata = os.environ.get("APPDATA")
    if not appdata:
        raise SystemExit("APPDATA 不可用")
    return Path(appdata) / "taskcap" / "feishu.json"


def read_existing() -> dict:
    path = target_path()
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8")) or {}
    except ValueError:
        return {}


def resolve_credentials(args, existing: dict) -> tuple:
    app_id = (args.app_id or os.environ.get("FEISHU_APP_ID", "")).strip()
    app_secret = (args.app_secret or os.environ.get("FEISHU_APP_SECRET", "")).strip()
    if not (app_id and app_secret) and args.from_file:
        source = Path(args.from_file)
        if not source.exists():
            raise SystemExit(f"找不到凭证文件：{source}")
        data = json.loads(source.read_text(encoding="utf-8"))
        app_id = app_id or (data.get("app_id") or "").strip()
        app_secret = app_secret or (data.get("app_secret") or "").strip()
    if not (app_id and app_secret):
        app_id = app_id or (existing.get("appId") or "").strip()
        app_secret = app_secret or (existing.get("appSecret") or "").strip()
    if not app_id or not app_secret:
        raise SystemExit(
            "缺少凭证：用 --app-id/--app-secret、环境变量 FEISHU_APP_ID/FEISHU_APP_SECRET，"
            "或 --from 指定凭证 JSON；也可以直接在 TaskCap 设置页填写"
        )
    return app_id, app_secret


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--disable", action="store_true", help="只写凭证，不启用自动同步")
    ap.add_argument("--enable", action="store_true", help="写入后启用自动同步")
    ap.add_argument("--interval", type=int, default=15, help="自动同步间隔（分钟）")
    ap.add_argument("--app-id", default="", help="飞书应用 App ID")
    ap.add_argument("--app-secret", default="", help="飞书应用 App Secret")
    ap.add_argument("--from", dest="from_file", default="", help="凭证 JSON 文件路径")
    ap.add_argument("--app-token", default="", help="多维表格 base token")
    ap.add_argument("--table-id", default="", help="数据表 ID")
    args = ap.parse_args()

    existing = read_existing()
    app_id, app_secret = resolve_credentials(args, existing)

    app_token = (
        args.app_token.strip()
        or os.environ.get("TASKCAP_APP_TOKEN", "").strip()
        or (existing.get("appToken") or "").strip()
    )
    table_id = (
        args.table_id.strip()
        or os.environ.get("TASKCAP_TABLE_ID", "").strip()
        or (existing.get("tableId") or "").strip()
    )

    config = {
        "enabled": args.enable or (not args.disable and bool(existing.get("enabled", False))),
        "appId": app_id,
        "appSecret": app_secret,
        "appToken": app_token,
        "tableId": table_id,
        "autoSyncMinutes": max(5, min(240, args.interval)),
        "lastSyncAt": existing.get("lastSyncAt"),
        "lastSyncSummary": existing.get("lastSyncSummary"),
    }
    target = target_path()
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"已写入：{target}")
    print(
        f"启用同步：{config['enabled']}｜目标表：{app_token or '未设置'} / {table_id or '未设置'}"
        f"｜间隔：{config['autoSyncMinutes']} 分钟"
    )


if __name__ == "__main__":
    main()
