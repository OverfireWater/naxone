#!/usr/bin/env python3
"""Publish a NaxOne release to Gitee + GitHub.

Gitee 用 REST API（需要 GITEE_TOKEN），GitHub 走本地 `gh` CLI（已 auth）。
两边都发：先 Gitee 成功，再 GitHub。任一失败会非零退出但不会回滚另一边。

Usage:
  export GITEE_TOKEN=...   (or release.env.local)
  python scripts/release_gitee.py

Optional:
  --version 0.2.1
  --file path/to/installer.exe
  --notes-file RELEASE_NOTES.md
  --dry-run
  --skip-gitee       # 只发 GitHub
  --skip-github      # 只发 Gitee（老版本兼容）
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
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
REPO = "naxone"
GH_REPO = "OverfireWater/naxone"
GH_FALLBACK_PATH = r"C:\Program Files\GitHub CLI\gh.exe"


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


def upload_asset_as(session: requests.Session, release_id: int, file_path: Path, name: str) -> dict[str, Any]:
    """跟 upload_asset 一样，但用指定 name 上传（让本地 latest-gitee-x.y.z.json 在远端叫 latest.json）。"""
    with file_path.open("rb") as f:
        files = {"file": (name, f, "application/octet-stream")}
        return request_json(session, "POST", f"{API}/repos/{OWNER}/{REPO}/releases/{release_id}/attach_files", files=files)


def find_gh() -> str | None:
    """定位 gh CLI：先看 PATH，再回退到 winget 默认安装路径。"""
    on_path = shutil.which("gh")
    if on_path:
        return on_path
    if Path(GH_FALLBACK_PATH).exists():
        return GH_FALLBACK_PATH
    return None


def publish_github(tag: str, version: str, installer: Path, notes_path: Path,
                   sig_path: Path | None = None, latest_json: str | None = None) -> None:
    """通过本地 gh CLI 发布 GitHub Release。已存在则刷新 notes 和资产。
    sig_path / latest_json 一并上传以支持 Tauri 自动更新。"""
    gh = find_gh()
    if gh is None:
        raise SystemExit("找不到 gh CLI（PATH 和 C:\\Program Files\\GitHub CLI 都没有）")

    title = f"NaxOne {version}"

    # 临时落盘 latest.json（gh CLI 只能上传文件路径）
    latest_tmp: Path | None = None
    if latest_json is not None:
        latest_tmp = installer.parent / "latest.json"
        latest_tmp.write_text(latest_json, encoding="utf-8")

    assets = [str(installer)]
    if sig_path is not None and sig_path.exists():
        assets.append(str(sig_path))
    if latest_tmp is not None:
        assets.append(str(latest_tmp))

    try:
        # 是否已存在
        exists = subprocess.run(
            [gh, "release", "view", tag, "--repo", GH_REPO],
            capture_output=True, text=True
        ).returncode == 0

        if exists:
            subprocess.run(
                [gh, "release", "edit", tag, "--repo", GH_REPO,
                 "--title", title, "--notes-file", str(notes_path)],
                check=True,
            )
            subprocess.run(
                [gh, "release", "upload", tag, *assets,
                 "--repo", GH_REPO, "--clobber"],
                check=True,
            )
            print(f"GitHub release {tag} 已更新")
        else:
            subprocess.run(
                [gh, "release", "create", tag, *assets,
                 "--repo", GH_REPO,
                 "--title", title,
                 "--notes-file", str(notes_path)],
                check=True,
            )
            print(f"GitHub release {tag} 已创建")
    finally:
        if latest_tmp is not None:
            latest_tmp.unlink(missing_ok=True)


def build_body(version: str, filename: str, sha256: str, file_size: float) -> str:
    return "\n".join([
        f"## NaxOne v{version}",
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
    parser.add_argument("--skip-gitee", action="store_true")
    parser.add_argument("--skip-github", action="store_true")
    args = parser.parse_args()

    version = args.version.strip()
    tag = f"v{version}"
    installer = Path(args.file_path) if args.file_path else ROOT / "target/release/bundle/nsis" / f"NaxOne_{version}_x64-setup.exe"
    if not installer.exists():
        raise SystemExit(f"安装包不存在: {installer}")

    # Tauri updater 签名：cargo tauri build 当前不会自动签 NSIS exe，
    # 这里在 .sig 不存在时调 cargo tauri signer sign 主动签一次。
    sig_path = installer.with_suffix(installer.suffix + ".sig")
    if not sig_path.exists():
        key_path = ROOT / "naxone-updater.key"
        if not key_path.exists():
            raise SystemExit(f"私钥不存在: {key_path}")
        print(f"[sign] 用 cargo tauri signer sign 给 {installer.name} 签名...")
        sign_cmd = [
            "cargo", "tauri", "signer", "sign",
            "-f", str(key_path),
            "-p", "",
            str(installer),
        ]
        result = subprocess.run(sign_cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise SystemExit(f"签名失败: {result.stderr or result.stdout}")
        if not sig_path.exists():
            raise SystemExit(f"签名后 .sig 仍不存在: {sig_path}")
    signature = sig_path.read_text(encoding="utf-8").strip()

    sha256 = sha256_of(installer)
    file_size = size_mb(installer)
    body = build_body(version, installer.name, sha256, file_size)
    notes_path: Path | None = None
    if args.notes_file:
        notes_path = Path(args.notes_file)
        body = notes_path.read_text(encoding="utf-8")

    # latest.json 内容（每个平台 url 不同，分别为 Gitee 和 GitHub 生成）
    pub_date = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    notes_text = body if args.notes_file else f"NaxOne v{version}"

    def make_latest_json(url: str) -> str:
        return json.dumps({
            "version": f"v{version}",
            "notes": notes_text,
            "pub_date": pub_date,
            "platforms": {
                "windows-x86_64": {
                    "signature": signature,
                    "url": url,
                },
            },
        }, ensure_ascii=False, indent=2)

    gitee_dl = f"https://gitee.com/{OWNER}/{REPO}/releases/download/{tag}/{installer.name}"
    github_dl = f"https://github.com/{GH_REPO}/releases/download/{tag}/{installer.name}"
    gitee_latest = make_latest_json(gitee_dl)
    github_latest = make_latest_json(github_dl)

    print(f"version: {version}")
    print(f"tag: {tag}")
    print(f"file: {installer}")
    print(f"sig:  {sig_path}  ({len(signature)} chars)")
    print(f"sha256: {sha256}")

    if args.dry_run:
        print("dry-run: skip API calls")
        return 0

    if not args.skip_gitee:
        token = load_token()
        session = requests.Session()
        session.headers.update({
            "Authorization": f"token {token}",
            "User-Agent": "NaxOne-Release/1.0",
            "Accept": "application/json",
        })

        release = find_release(session, tag)
        if release is None:
            release = create_release(session, tag, f"NaxOne v{version}", body)
            print(f"Gitee release 已创建 id={release['id']}")
        else:
            release = update_release(session, release["id"], f"NaxOne v{version}", body)
            print(f"Gitee release 已更新 id={release['id']}")

        release_id = release["id"]
        # 删旧资产：installer / .sig / latest.json 三类同名都清掉再上新
        target_names = {installer.name, sig_path.name, "latest.json"}
        for asset in list_assets(session, release_id):
            if asset.get("name") in target_names:
                delete_asset(session, release_id, asset["id"])
                print(f"Gitee 删除旧资产 {asset.get('name')} id={asset['id']}")

        uploaded = upload_asset(session, release_id, installer)
        print(f"Gitee 上传资产: {uploaded.get('name') or installer.name}")

        # 上传 .sig
        uploaded = upload_asset(session, release_id, sig_path)
        print(f"Gitee 上传资产: {uploaded.get('name') or sig_path.name}")

        # 上传 latest.json（Gitee 版 url 指向 Gitee）
        gitee_latest_path = ROOT / f".latest-gitee-{version}.tmp.json"
        gitee_latest_path.write_text(gitee_latest, encoding="utf-8")
        try:
            uploaded = upload_asset_as(session, release_id, gitee_latest_path, "latest.json")
            print(f"Gitee 上传资产: latest.json")
        finally:
            gitee_latest_path.unlink(missing_ok=True)

    if not args.skip_github:
        # GitHub 需要 notes 文件路径，没传 --notes-file 时临时落盘
        if notes_path is None:
            notes_path = ROOT / f".release-notes-{version}.tmp.md"
            notes_path.write_text(body, encoding="utf-8")
            try:
                publish_github(tag, version, installer, notes_path, sig_path, github_latest)
            finally:
                notes_path.unlink(missing_ok=True)
        else:
            publish_github(tag, version, installer, notes_path, sig_path, github_latest)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
