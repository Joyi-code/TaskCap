#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""检查 TaskCap任务 表的数据是否同步正常（只读）。

目标解析顺序：--app/--table → 环境变量 FEISHU_APP_TOKEN / FEISHU_TABLE_ID →
本机 %APPDATA%\\taskcap\\feishu.json。脚本不内置任何租户 token。

用法：
    python scripts/feishu/verify.py
    python scripts/feishu/verify.py --limit 10
"""
import argparse
import os
import sys
from collections import Counter
from datetime import datetime, timezone, timedelta
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import feishu_client as fs  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")

TZ = timezone(timedelta(hours=8))


def resolve_targets(app_arg: str, table_arg: str) -> tuple:
    config = fs.load_taskcap_config()
    app_token = (
        (app_arg or "").strip()
        or os.environ.get("FEISHU_APP_TOKEN", "").strip()
        or (config.get("appToken") or "").strip()
    )
    table_id = (
        (table_arg or "").strip()
        or os.environ.get("FEISHU_TABLE_ID", "").strip()
        or (config.get("tableId") or "").strip()
    )
    if not app_token or not table_id:
        raise SystemExit("缺少目标表：本机 feishu.json 未配置，或用 --app/--table 指定")
    return app_token, table_id


def show(value, limit=60):
    text = fs.flat(value)
    if len(text) > limit:
        text = text[:limit] + "…"
    return text


def as_date(value):
    if not isinstance(value, (int, float)):
        return ""
    return datetime.fromtimestamp(value / 1000, TZ).strftime("%Y-%m-%d %H:%M")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="", help="多维表格 base token")
    ap.add_argument("--table", default="", help="数据表 ID")
    ap.add_argument("--limit", type=int, default=3, help="打印样本条数")
    args = ap.parse_args()

    app_token, table_id = resolve_targets(args.app, args.table)
    tk = fs.get_token()
    records = fs.list_records(app_token, table_id, tk)
    print(f"记录总数：{len(records)}")

    status = Counter(show(r["fields"].get("状态")) for r in records)
    priority = Counter(show(r["fields"].get("优先级")) for r in records)
    with_due = sum(1 for r in records if r["fields"].get("截止日期"))
    missing_id = sum(1 for r in records if not show(r["fields"].get("本地ID")).strip())
    print(f"状态分布：{dict(status)}")
    print(f"优先级分布：{dict(priority)}")
    print(f"有截止日期：{with_due} 条；缺本地ID：{missing_id} 条")

    print("\n样本：")
    for record in records[: args.limit]:
        fields = record["fields"]
        print(f"- {show(fields.get('任务标题'), 40)}")
        print(
            f"  状态={show(fields.get('状态'))} 优先级={show(fields.get('优先级'))} "
            f"项目={show(fields.get('项目'))} 标签={show(fields.get('标签'))}"
        )
        print(
            f"  创建={as_date(fields.get('创建时间'))} 截止={as_date(fields.get('截止日期'))} "
            f"完成={as_date(fields.get('完成时间'))}"
        )
        print(
            f"  预计={show(fields.get('预计分钟'))} 专注={show(fields.get('专注分钟'))} "
            f"推迟={show(fields.get('推迟次数'))} 子任务={show(fields.get('子任务'))}"
        )
        print(
            f"  今日清单={show(fields.get('今日清单'))} 当前任务={show(fields.get('当前任务'))} "
            f"本地ID={show(fields.get('本地ID'))}"
        )


if __name__ == "__main__":
    main()
