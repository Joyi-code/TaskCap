#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""在指定多维表格里创建 TaskCap 同步表（幂等：已有同名表直接返回）。

目标 base 解析顺序：--app → 环境变量 FEISHU_APP_TOKEN → 本机
%APPDATA%\\taskcap\\feishu.json 的 appToken。脚本不内置任何租户 token。

用法：
    python scripts/feishu/create_table.py --app <app_token>
    python scripts/feishu/create_table.py --fields
"""
import argparse
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import feishu_client as fs  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")

TABLE_NAME = "TaskCap任务"

DATE = {"date_formatter": "yyyy/MM/dd", "auto_fill": False}
DATETIME = {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": False}

FIELDS = [
    {"field_name": "任务标题", "type": 1},  # 主字段（第一列）
    {"field_name": "本地ID", "type": 1},
    {
        "field_name": "状态",
        "type": 3,
        "property": {"options": [{"name": "待办", "color": 0}, {"name": "已完成", "color": 2}]},
    },
    {
        "field_name": "优先级",
        "type": 3,
        "property": {
            "options": [
                {"name": "高", "color": 1},
                {"name": "中", "color": 3},
                {"name": "低", "color": 5},
            ]
        },
    },
    {"field_name": "项目", "type": 1},
    {"field_name": "标签", "type": 1},
    {"field_name": "截止日期", "type": 5, "property": DATE},
    {"field_name": "提醒时间", "type": 5, "property": DATETIME},
    {"field_name": "完成时间", "type": 5, "property": DATETIME},
    {"field_name": "创建时间", "type": 5, "property": DATETIME},
    {"field_name": "更新时间", "type": 5, "property": DATETIME},
    {"field_name": "预计分钟", "type": 2},
    {"field_name": "专注分钟", "type": 2},
    {"field_name": "推迟次数", "type": 2},
    {"field_name": "子任务", "type": 1},
    {"field_name": "备注", "type": 1},
    {"field_name": "重复规则", "type": 1},
    {"field_name": "今日清单", "type": 7},
    {"field_name": "当前任务", "type": 7},
    {"field_name": "同步时间", "type": 5, "property": DATETIME},
]


def resolve_app(cli_value: str) -> str:
    token = (
        (cli_value or "").strip()
        or os.environ.get("FEISHU_APP_TOKEN", "").strip()
        or (fs.load_taskcap_config().get("appToken") or "").strip()
    )
    if not token:
        raise SystemExit("缺少 base token：用 --app 指定，或先在本机 feishu.json 里配好 appToken")
    return token


def find_table(tables: list):
    for table in tables:
        if (table.get("name") or "").strip() == TABLE_NAME:
            return table.get("table_id")
    return None


def create_table(app_token: str):
    tk = fs.get_token()
    existing = find_table(fs.list_tables(app_token, tk))
    if existing:
        print(f"表已存在：{TABLE_NAME} -> {existing}")
        return existing

    payload = {
        "table": {
            "name": TABLE_NAME,
            "default_view_name": "全部任务",
            "fields": FIELDS,
        }
    }
    body = fs.request(f"/bitable/v1/apps/{app_token}/tables", "POST", payload, tk)
    if body.get("code") != 0:
        raise SystemExit(f"创建表失败：{json.dumps(body, ensure_ascii=False)}")
    table_id = body["data"]["table_id"]
    print(f"已创建：{TABLE_NAME} -> {table_id}")
    return table_id


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="", help="目标多维表格 base token")
    ap.add_argument("--fields", action="store_true", help="打印字段结构")
    args = ap.parse_args()

    app_token = resolve_app(args.app)

    if args.fields:
        tk = fs.get_token()
        table_id = find_table(fs.list_tables(app_token, tk))
        if not table_id:
            raise SystemExit("表不存在，先运行一次创建")
        for field in fs.list_fields(app_token, table_id, tk):
            options = [o["name"] for o in ((field.get("property") or {}).get("options") or [])]
            print(f"{field['field_name']:<10} type={field['type']:<5} {options if options else ''}")
        return

    create_table(app_token)


if __name__ == "__main__":
    main()
