#!/usr/bin/env python3
"""
Universal AI Assistant - Software Bill of Materials (SBOM) Generator (SPDX 2.3)
Subphase: S11.P2.1

Parses Rust dependencies from core/Cargo.lock and Flutter/Dart dependencies
from app/pubspec.lock to produce valid, standardized SPDX 2.3 JSON SBOM documents.
"""

import argparse
import datetime
import json
import os
import re
import sys
import tomllib


def sanitize_spdx_id(name: str, version: str) -> str:
    cleaned_name = re.sub(r"[^a-zA-Z0-9.\-]", "-", name)
    cleaned_ver = re.sub(r"[^a-zA-Z0-9.\-]", "-", version)
    return f"SPDXRef-Package-{cleaned_name}-{cleaned_ver}"


def parse_cargo_lock(lock_path: str) -> list[dict]:
    if not os.path.isfile(lock_path):
        print(f"Warning: Cargo lockfile not found at {lock_path}", file=sys.stderr)
        return []

    with open(lock_path, "rb") as f:
        data = tomllib.load(f)

    packages = []
    for pkg in data.get("package", []):
        name = pkg.get("name", "")
        version = pkg.get("version", "")
        source = pkg.get("source", "")
        checksum = pkg.get("checksum", "")

        spdx_id = sanitize_spdx_id(name, version)
        pkg_entry = {
            "SPDXID": spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": source if source else "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "supplier": "Organization: crates.io" if "crates.io" in source else "Organization: Universal AI Assistant",
        }
        if checksum:
            pkg_entry["checksums"] = [
                {
                    "algorithm": "SHA256",
                    "checksumValue": checksum,
                }
            ]
        packages.append(pkg_entry)

    return sorted(packages, key=lambda x: x["name"])


def parse_pubspec_lock(lock_path: str) -> list[dict]:
    if not os.path.isfile(lock_path):
        print(f"Warning: pubspec.lock not found at {lock_path}", file=sys.stderr)
        return []

    with open(lock_path, "r", encoding="utf-8") as f:
        content = f.read()

    packages = []
    current_pkg = None
    current_info = {}

    for line in content.splitlines():
        pkg_match = re.match(r"^  ([a-zA-Z0-9_]+):", line)
        if pkg_match:
            if current_pkg and "version" in current_info:
                packages.append((current_pkg, current_info))
            current_pkg = pkg_match.group(1)
            current_info = {}
            continue

        if current_pkg:
            v_match = re.search(r'version:\s*"?([^"\s]+)"?', line)
            if v_match:
                current_info["version"] = v_match.group(1)
            s_match = re.search(r'sha256:\s*"?([a-f0-9]+)"?', line)
            if s_match:
                current_info["sha256"] = s_match.group(1)
            u_match = re.search(r'url:\s*"?([^"\s]+)"?', line)
            if u_match:
                current_info["url"] = u_match.group(1)
            d_match = re.search(r'dependency:\s*"?([^"\s]+)"?', line)
            if d_match:
                current_info["dependency"] = d_match.group(1)

    if current_pkg and "version" in current_info:
        packages.append((current_pkg, current_info))

    spdx_packages = []
    for name, info in packages:
        version = info.get("version", "0.0.0")
        sha256 = info.get("sha256", "")
        url = info.get("url", "https://pub.dev")
        spdx_id = sanitize_spdx_id(name, version)

        pkg_entry = {
            "SPDXID": spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": url,
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "supplier": "Organization: pub.dev",
        }
        if sha256:
            pkg_entry["checksums"] = [
                {
                    "algorithm": "SHA256",
                    "checksumValue": sha256,
                }
            ]
        spdx_packages.append(pkg_entry)

    return sorted(spdx_packages, key=lambda x: x["name"])


def build_spdx_document(name: str, doc_namespace: str, packages: list[dict]) -> dict:
    timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    doc_id = "SPDXRef-DOCUMENT"

    relationships = [
        {
            "spdxElementId": doc_id,
            "relatedSpdxElement": pkg["SPDXID"],
            "relationshipType": "DESCRIBES",
        }
        for pkg in packages
    ]

    return {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": doc_id,
        "name": name,
        "documentNamespace": doc_namespace,
        "creationInfo": {
            "created": timestamp,
            "creators": [
                "Tool: aissistant-sbom-generator-1.0.0",
                "Organization: Universal AI Assistant Team",
            ],
        },
        "packages": packages,
        "relationships": relationships,
    }


def main():
    parser = argparse.ArgumentParser(description="Generate SPDX 2.3 SBOM for Universal AI Assistant")
    parser.add_argument("--output-dir", default="dist/sbom", help="Directory where SBOM files will be written")
    parser.add_argument("--cargo-lock", default="core/Cargo.lock", help="Path to Cargo.lock")
    parser.add_argument("--pubspec-lock", default="app/pubspec.lock", help="Path to pubspec.lock")
    args = parser.parse_args()

    os.makedirs(args.output_dir, exist_ok=True)

    # 1. Rust Core SBOM
    rust_pkgs = parse_cargo_lock(args.cargo_lock)
    rust_doc = build_spdx_document(
        name="Universal AI Assistant - Rust Core",
        doc_namespace="https://github.com/aissistant/aissistant/spdx/rust-core-1.0.0",
        packages=rust_pkgs,
    )
    rust_out = os.path.join(args.output_dir, "rust-sbom.spdx.json")
    with open(rust_out, "w", encoding="utf-8") as f:
        json.dump(rust_doc, f, indent=2)
    print(f"Generated Rust SPDX SBOM with {len(rust_pkgs)} packages -> {rust_out}")

    # 2. Flutter/Dart SBOM
    flutter_pkgs = parse_pubspec_lock(args.pubspec_lock)
    flutter_doc = build_spdx_document(
        name="Universal AI Assistant - Flutter App",
        doc_namespace="https://github.com/aissistant/aissistant/spdx/flutter-app-1.0.0",
        packages=flutter_pkgs,
    )
    flutter_out = os.path.join(args.output_dir, "flutter-sbom.spdx.json")
    with open(flutter_out, "w", encoding="utf-8") as f:
        json.dump(flutter_doc, f, indent=2)
    print(f"Generated Flutter SPDX SBOM with {len(flutter_pkgs)} packages -> {flutter_out}")

    # 3. Unified SBOM
    combined_pkgs = sorted(rust_pkgs + flutter_pkgs, key=lambda x: (x["name"], x["versionInfo"]))
    unified_doc = build_spdx_document(
        name="Universal AI Assistant - Full Stack Unified",
        doc_namespace="https://github.com/aissistant/aissistant/spdx/unified-stack-1.0.0",
        packages=combined_pkgs,
    )
    unified_out = os.path.join(args.output_dir, "unified-sbom.spdx.json")
    with open(unified_out, "w", encoding="utf-8") as f:
        json.dump(unified_doc, f, indent=2)
    print(f"Generated Unified SPDX SBOM with {len(combined_pkgs)} packages -> {unified_out}")


if __name__ == "__main__":
    main()
