#!/usr/bin/env python3
"""box 카탈로그(JSON)에 새 버전을 더한다. 기존 버전은 남긴다.

    box/publish_catalog.py --catalog 현재.json --version 1.1.0 \\
        --url-template 'https://.../{version}/Ubuntu-k8s-{version}-virtualbox-{arch}.box' \\
        --sha arm64=<sha256> --sha amd64=<sha256> [--description "..."] > 새.json

같은 버전이 이미 있으면 바꾼다(다시 실행해도 된다). 베이그런트는 카탈로그에서 가장 높은 버전을 고르므로
기존 버전을 남겨도 새로 받는 사람은 새 버전을 받는다. box_version으로 고정한 Vagrantfile은 계속 동작한다.
"""
import argparse
import json
import sys

# 1.0.0과 같은 순서와 기본값. arm64가 기본 아키텍처다
ARCHES = [("arm64", True), ("amd64", False)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--catalog", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--url-template", required=True)
    ap.add_argument("--sha", action="append", required=True, help="arch=sha256")
    ap.add_argument("--description", default="")
    a = ap.parse_args()

    shas = dict(s.split("=", 1) for s in a.sha)
    missing = [arch for arch, _ in ARCHES if arch not in shas]
    if missing:
        sys.exit(f"SHA-256이 없는 아키텍처: {missing}")

    with open(a.catalog) as f:
        cat = json.load(f)

    entry = {
        "version": a.version,
        "description": a.description,
        "providers": [
            {
                "name": "virtualbox",
                "architecture": arch,
                "default_architecture": default,
                "checksum_type": "sha256",
                "checksum": shas[arch],
                "url": a.url_template.format(version=a.version, arch=arch),
            }
            for arch, default in ARCHES
        ],
    }
    others = [v for v in cat.get("versions", []) if v.get("version") != a.version]
    cat["versions"] = [entry] + others
    json.dump(cat, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
