#!/usr/bin/env python3
"""
Universal AI Assistant - Release Artifact Checksum Aggregator & Verifier
Subphase: S11.P2.1

Aggregates SHA-256 manifests across desktop, Android, and iOS distribution targets
into a unified SHA256SUMS.txt manifest and provides cryptographic integrity verification.
"""

import argparse
import hashlib
import os
import sys


def compute_sha256(filepath: str) -> str:
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def aggregate_manifests(dist_dir: str) -> dict[str, str]:
    checksum_map: dict[str, str] = {}
    known_manifests = [
        "SHA256SUMS_DESKTOP.txt",
        "SHA256SUMS_ANDROID.txt",
        "SHA256SUMS_IOS.txt",
    ]

    for manifest_name in known_manifests:
        manifest_path = os.path.join(dist_dir, manifest_name)
        if os.path.isfile(manifest_path):
            with open(manifest_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith("#"):
                        continue
                    parts = line.split(maxsplit=1)
                    if len(parts) == 2:
                        hash_val, file_val = parts[0].strip(), parts[1].strip().lstrip("*")
                        base_file = os.path.basename(file_val)
                        checksum_map[base_file] = hash_val.lower()

    # Also directly scan for distribution binaries in dist_dir if any exist
    release_extensions = (
        ".exe", ".msix", ".zip", ".deb", ".AppImage", ".dmg",
        ".apk", ".aab", ".ipa", ".spdx.json"
    )
    for root, _, files in os.walk(dist_dir):
        for f in files:
            if any(f.endswith(ext) for ext in release_extensions):
                rel_path = os.path.relpath(os.path.join(root, f), dist_dir)
                if rel_path not in checksum_map and f not in checksum_map:
                    full_p = os.path.join(root, f)
                    calculated = compute_sha256(full_p)
                    checksum_map[rel_path] = calculated

    return checksum_map


def write_unified_manifest(dist_dir: str, checksum_map: dict[str, str]) -> str:
    manifest_path = os.path.join(dist_dir, "SHA256SUMS.txt")
    with open(manifest_path, "w", encoding="utf-8") as f:
        for fname in sorted(checksum_map.keys()):
            f.write(f"{checksum_map[fname]}  {fname}\n")
    return manifest_path


def verify_manifest(dist_dir: str, manifest_path: str) -> bool:
    if not os.path.isfile(manifest_path):
        print(f"Error: Manifest file not found at {manifest_path}", file=sys.stderr)
        return False

    all_ok = True
    checked_count = 0

    with open(manifest_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(maxsplit=1)
            if len(parts) != 2:
                continue

            expected_hash = parts[0].strip().lower()
            rel_file = parts[1].strip().lstrip("*")
            full_path = os.path.join(dist_dir, rel_file)

            if not os.path.isfile(full_path):
                # Search by basename
                candidate = os.path.join(dist_dir, os.path.basename(rel_file))
                if os.path.isfile(candidate):
                    full_path = candidate
                else:
                    # File not present on this host
                    continue

            actual_hash = compute_sha256(full_path).lower()
            checked_count += 1

            if actual_hash == expected_hash:
                print(f"[OK] {rel_file}: SHA-256 verified ({actual_hash[:16]}...)")
            else:
                print(f"[FAIL] {rel_file}: Mismatch! Expected {expected_hash}, got {actual_hash}", file=sys.stderr)
                all_ok = False

    print(f"Verification finished: {checked_count} file(s) checked.")
    return all_ok and checked_count > 0


def main():
    parser = argparse.ArgumentParser(description="Aggregate and verify Universal AI Assistant release checksums")
    parser.add_argument("--dist-dir", default="dist", help="Path to distribution directory")
    parser.add_argument("--verify", action="store_true", help="Verify existing files against SHA256SUMS.txt")
    args = parser.parse_args()

    os.makedirs(args.dist_dir, exist_ok=True)

    checksum_map = aggregate_manifests(args.dist_dir)
    manifest_path = write_unified_manifest(args.dist_dir, checksum_map)
    print(f"Aggregated {len(checksum_map)} release artifacts into {manifest_path}")

    if args.verify:
        success = verify_manifest(args.dist_dir, manifest_path)
        if not success:
            sys.exit(1)


if __name__ == "__main__":
    main()
