#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""飞书多维表格标准库客户端：不依赖 requests，也不依赖任何个人目录。

凭证来源优先级：
1. 环境变量 `FEISHU_APP_ID` / `FEISHU_APP_SECRET`
2. `%APPDATA%\\taskcap\\feishu.json` 里的 `appId` / `appSecret`（TaskCap 设置页保存的）
"""
import json
import os
import ssl
import sys
import urllib.error
import urllib.request
from pathlib import Path

API = "https://open.feishu.cn/open-apis"

_CTX = ssl.create_default_context()
_CTX.check_hostname = False
_CTX.verify_mode = ssl.CERT_NONE


def taskcap_config_path() -> Path:
    appdata = os.environ.get("APPDATA")
    if not appdata:
        raise SystemExit("APPDATA 不可用")
    return Path(appdata) / "taskcap" / "feishu.json"


def load_taskcap_config() -> dict:
    path = taskcap_config_path()
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8")) or {}
    except ValueError:
        return {}


def resolve_credentials() -> tuple:
    app_id = os.environ.get("FEISHU_APP_ID", "").strip()
    secret = os.environ.get("FEISHU_APP_SECRET", "").strip()
    if not (app_id and secret):
        cfg = load_taskcap_config()
        app_id = app_id or (cfg.get("appId") or "").strip()
        secret = secret or (cfg.get("appSecret") or "").strip()
    if not app_id or not secret:
        raise SystemExit(
            "缺少飞书应用凭证：设置环境变量 FEISHU_APP_ID / FEISHU_APP_SECRET，"
            "或先在 TaskCap 设置页填好 App ID / App Secret"
        )
    return app_id, secret


def request(path, method="GET", payload=None, token=None, timeout=40):
    """调用飞书 OpenAPI，返回解析后的 JSON（HTTP 错误也返回 body）。"""
    url = path if path.startswith("http") else API + path
    data = None
    if payload is not None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json; charset=utf-8")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=_CTX) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", "replace")
        try:
            return json.loads(body)
        except ValueError:
            return {"code": -1, "msg": f"HTTP {error.code}: {body[:300]}"}


def get_token() -> str:
    app_id, app_secret = resolve_credentials()
    body = request(
        "/auth/v3/tenant_access_token/internal",
        "POST",
        {"app_id": app_id, "app_secret": app_secret},
    )
    if body.get("code") != 0:
        raise SystemExit("获取 tenant_access_token 失败：%s" % json.dumps(body, ensure_ascii=False))
    return body["tenant_access_token"]


def list_tables(app_token: str, token: str = "") -> list:
    tk = token or get_token()
    body = request(f"/bitable/v1/apps/{app_token}/tables?page_size=100", token=tk)
    if body.get("code") != 0:
        raise SystemExit("list_tables 失败：%s" % json.dumps(body, ensure_ascii=False))
    return (body.get("data") or {}).get("items") or []


def list_fields(app_token: str, table_id: str, token: str = "") -> list:
    tk = token or get_token()
    body = request(
        f"/bitable/v1/apps/{app_token}/tables/{table_id}/fields?page_size=100", token=tk
    )
    if body.get("code") != 0:
        raise SystemExit("list_fields 失败：%s" % json.dumps(body, ensure_ascii=False))
    return (body.get("data") or {}).get("items") or []


def list_records(app_token: str, table_id: str, token: str = "", page_size: int = 500) -> list:
    tk = token or get_token()
    out, page_token = [], None
    while True:
        payload = {"automatic_fields": False}
        if page_token:
            payload["page_token"] = page_token
        body = request(
            f"/bitable/v1/apps/{app_token}/tables/{table_id}/records/search?page_size={page_size}",
            "POST",
            payload,
            tk,
        )
        if body.get("code") != 0:
            raise SystemExit("list_records 失败：%s" % json.dumps(body, ensure_ascii=False))
        data = body.get("data") or {}
        out.extend(data.get("items") or [])
        if not data.get("has_more"):
            return out
        page_token = data.get("page_token")


def delete_records(app_token: str, table_id: str, record_ids: list, token: str = "") -> None:
    tk = token or get_token()
    for start in range(0, len(record_ids), 100):
        part = record_ids[start : start + 100]
        body = request(
            f"/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_delete",
            "POST",
            {"records": part},
            tk,
        )
        if body.get("code") != 0:
            raise SystemExit("batch_delete 失败：%s" % json.dumps(body, ensure_ascii=False))


def flat(value) -> str:
    """把飞书返回的字段值压平成字符串（文本/单选/数字/嵌套文本通吃）。"""
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, dict):
        return value.get("text") or value.get("name") or value.get("en_name") or ""
    if isinstance(value, list):
        parts = []
        for item in value:
            if isinstance(item, dict):
                parts.append(item.get("text") or item.get("name") or item.get("en_name") or "")
            else:
                parts.append(str(item))
        return " ".join(part for part in parts if part)
    return str(value)
