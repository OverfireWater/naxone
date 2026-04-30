#!/usr/bin/env python3
"""Publish a RustStudy release to Gitee.

Usage:
  export GITEE_TOKEN=...
  python scripts/release_gitee.py

Optional:
  --version 0.2.1
  --file path/to/installer.exe
  --notes-file RELEASE_NOTES.md
  --dry-run
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests

ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"
DEFAULT_TOKEN_FILE = ROOT / "release.env.local"
API = "https://gitee.com/api/v5"
OWNER = "kz_y"
REPO = "ruststudy"


def read_version() -> str:
    text = CARGO_TOML.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"\s*$', text, re.MULTILINE)
    if not m:
        raise SystemExit("无法从 Cargo.toml 读取版本号")
    return m.group(1)


def load_token() -> str:
    token = os.environ.get("GITEE_TOKEN", "").strip()
    if token:
        return token
    if DEFAULT_TOKEN_FILE.exists():
        for line in DEFAULT_TOKEN_FILE.read_text(encoding="utf-8").splitlines():
            if line.startswith("GITEE_TOKEN="):
                return line.split("=", 1)[1].strip()
    raise SystemExit("缺少 GITEE_TOKEN")


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def size_mb(path: Path) -> float:
    return path.stat().st_size / 1024 / 1024


def request_json(session: requests.Session, method: str, url: str, **kwargs: Any) -> Any:
    resp = session.request(method, url, timeout=60, **kwargs)
    resp.raise_for_status()
    if resp.status_code == 204:
        return None
    if not resp.text:
        return None
    return resp.json()


def find_release(session: requests.Session, tag: str) -> dict[str, Any] | None:
    try:
        return request_json(session, "GET", f"{API}/repos/{OWNER}/{REPO}/releases/tags/{tag}")
    except requests.HTTPError as e:
        if getattr(e.response, "status_code", None) == 404:
            return None
        raise


def list_assets(session: requests.Session, release_id: int) -> list[dict[str, Any]]:
    data = request_json(session, "GET", f"{API}/repos/{OWNER}/{REPO}/releases/{release_id}/attach_files")
    return data or []


def delete_asset(session: requests.Session, release_id: int, asset_id: int) -> None:
    request_json(session, "DELETE", f"{API}/repos/{OWNER}/{REPO}/releases/{release_id}/attach_files/{asset_id}")


def create_release(session: requests.Session, tag: str, name: str, body: str) -> dict[str, Any]:
    payload = {
        "access_token": session.headers.get("Authorization", "").split(" ", 1)[-1],
        "tag_name": tag,
        "name": name,
        "body": body,
        "target_commitish": "main",
        "prerelease": False,
    }
    return request_json(session, "POST", f"{API}/repos/{OWNER}/{REPO}/releases", data=payload)


def update_release(session: requests.Session, release_id: int, name: str, body: str) -> dict[str, Any]:
    payload = {
        "access_token": session.headers.get("Authorization", "").split(" ", 1)[-1],
        "name": name,
        "body": body,
    }
    return request_json(session, "PATCH", f"{API}/repos/{OWNER}/{REPO}/releases/{release_id}", data=payload)


def upload_asset(session: requests.Session, release_id: int, file_path: Path) -> dict[str, Any]:
    with file_path.open("rb") as f:
        files = {"file": (file_path.name, f, "application/octet-stream")}
        return request_json(session, "POST", f"{API}/repos/{OWNER}/{REPO}/releases/{release_id}/attach_files", files=files)


def build_body(version: str, filename: str, sha256: str, file_size: float) -> str:
    return "\n".join([
        f"## RustStudy v{version}",
        "",
        f"- Built at: {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%SZ')}",
        f"- File: {filename}",
        f"- SHA256: `{sha256}`",
        f"- Size: {file_size:.2f} MB",
    ])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", default=read_version())
    parser.add_argument("--file", dest="file_path")
    parser.add_argument("--notes-file")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    version = args.version.strip()
    tag = f"v{version}"
    installer = Path(args.file_path) if args.file_path else ROOT / "target/release/bundle/nsis" / f"RustStudy_{version}_x64-setup.exe"
    if not installer.exists():
        raise SystemExit(f"安装包不存在: {installer}")

    token = load_token()
    session = requests.Session()
    session.headers.update({
        "Authorization": f"token {token}",
        "User-Agent": "RustStudy-Release/1.0",
        "Accept": "application/json",
    })

    sha256 = sha256_of(installer)
    file_size = size_mb(installer)
    body = build_body(version, installer.name, sha256, file_size)
    if args.notes_file:
        body = Path(args.notes_file).read_text(encoding="utf-8")

    print(f"version: {version}")
    print(f"tag: {tag}")
    print(f"file: {installer}")
    print(f"sha256: {sha256}")

    if args.dry_run:
        print("dry-run: skip API calls")
        return 0

    release = find_release(session, tag)
    if release is None:
        release = create_release(session, tag, f"RustStudy v{version}", body)
        print(f"created release id={release['id']}")
    else:
        release = update_release(session, release["id"], f"RustStudy v{version}", body)
        print(f"updated release id={release['id']}")

    release_id = release["id"]
    for asset in list_assets(session, release_id):
        if asset.get("name") == installer.name:
            delete_asset(session, release_id, asset["id"])
            print(f"deleted old asset id={asset['id']}")

    uploaded = upload_asset(session, release_id, installer)
    print(f"uploaded asset: {uploaded.get('name') or installer.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
