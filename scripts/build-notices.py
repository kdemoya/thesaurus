#!/usr/bin/env python3
"""Package third-party notices, original texts, and unchanged MPL source archives."""
import hashlib
import html
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "scripts/notices"
VERSION = "0.9.1"
ABOUT_SHA = "6a38fe166d17a674269d4373256c0b6bd93acc2553e12de0517cb9ecc73c9c02"
WHISPER_COMMIT = "d1be6fde11ac6e0407606b4e42fe72d34add8037"
WHISPER_SHA = "645445da884df6dbd52b30f82d7be7c01d1767c938714e477e786b88d529e9f4"
MODEL_LICENSE_SHA = "b5d65a59060e68c4ff940e1eddfa6f94b2d68fdf58ed7f4dd57721c997e35e9d"
LICENSE_NAME = re.compile(r"^(licen[sc]e|notice|copying|copyright|unlicense)([-._]|$)", re.I)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def checked(path, expected):
    if not path.is_file() or digest(path) != expected:
        raise RuntimeError(f"Missing or changed pinned input: {path.relative_to(ROOT)}")
    return path


def cargo_environment():
    env = os.environ.copy()
    if (ROOT / ".tools/cargo/bin/cargo").is_file():
        env["CARGO_HOME"] = str(ROOT / ".tools/cargo")
        env["RUSTUP_HOME"] = str(ROOT / ".tools/rustup")
        env["PATH"] = f'{ROOT}/.tools/cargo/bin:{env["PATH"]}'
    return env


def cargo_about(env):
    cache = ROOT / ".tools/cargo-about"
    cache.mkdir(parents=True, exist_ok=True)
    if platform.system() == "Darwin" and platform.machine() == "arm64":
        name = f"cargo-about-{VERSION}-aarch64-apple-darwin"
        archive = cache / f"{name}.tar.gz"
        if not archive.is_file() or digest(archive) != ABOUT_SHA:
            url = f"https://github.com/EmbarkStudios/cargo-about/releases/download/{VERSION}/{name}.tar.gz"
            partial = archive.with_suffix(".partial")
            with urllib.request.urlopen(url, timeout=120) as response, partial.open("wb") as output:
                shutil.copyfileobj(response, output)
            checked(partial, ABOUT_SHA)
            partial.replace(archive)
        binary = cache / name / "cargo-about"
        # Extract only the executable, avoiding archive path/symlink traversal.
        with tarfile.open(archive) as tar:
            member = tar.getmember(f"{name}/cargo-about")
            if not member.isfile():
                raise RuntimeError("cargo-about archive contains no regular executable")
            binary.parent.mkdir(exist_ok=True)
            with tar.extractfile(member) as source, binary.open("wb") as output:
                shutil.copyfileobj(source, output)
        binary.chmod(0o755)
    else:
        binary = cache / "source/bin/cargo-about"
        if not binary.is_file():
            subprocess.run([str(ROOT / "scripts/cargo.sh"), "install", "cargo-about", "--version", f"={VERSION}", "--locked", "--root", str(cache / "source"), "--target-dir", str(cache / "build")], env=env, check=True, cwd=ROOT)
    version = subprocess.check_output([str(binary), "--version"], text=True).strip()
    if version != f"cargo-about {VERSION}":
        raise RuntimeError(f"Unexpected notice generator: {version}")
    return binary


def notice_files(base):
    return sorted(p for p in base.rglob("*") if p.is_file() and LICENSE_NAME.match(p.name)
                  and p.suffix.lower() not in {".rs", ".c", ".cpp", ".h", ".sh"})


def main():
    profile = sys.argv[1] if len(sys.argv) == 2 else "debug"
    if profile not in {"debug", "release"}:
        raise RuntimeError("Usage: scripts/build-notices.sh [debug|release]")
    env = cargo_environment()
    binary = cargo_about(env)
    native_target = "aarch64-apple-darwin" if platform.machine() == "arm64" else "x86_64-apple-darwin"
    targets = [native_target, "wasm32-unknown-unknown"]
    parent = ROOT / "target" / profile
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="notices-", dir=parent) as tmp:
        dest = Path(tmp)
        raw = dest / "raw.json"
        # Build steps fetch Cargo dependencies first. License harvesting itself is
        # offline so upstream changes cannot silently change the shipped texts.
        subprocess.run([str(binary), "generate", "--locked", "--offline", "--fail", "--workspace", "--target", targets[0], "--target", targets[1], "--config", str(CONFIG / "about.toml"), "--format", "json", "--output-file", str(raw)], env=env, check=True, cwd=ROOT)
        about = json.loads(raw.read_text())
        raw.unlink()
        lock_bytes = (ROOT / "Cargo.lock").read_bytes()
        lock = tomllib.loads(lock_bytes.decode())
        checksums = {(p["name"], p["version"]): p.get("checksum") for p in lock["package"]}
        supplemental = {(p["crate"], p["version"]): p for p in json.loads((CONFIG / "upstream.json").read_text())}
        packages = {p["package"]["id"]: p["package"] for p in about["crates"]}
        selected = {}
        licenses = []
        for i, license in enumerate(about["licenses"]):
            used = []
            for use in license["used_by"]:
                package = use["crate"]
                if not package.get("source"):
                    continue
                selected.setdefault(package["id"], set()).add(license["id"])
                used.append({"name": package["name"], "version": package["version"]})
            if used:
                licenses.append({"id": license["id"], "name": license["name"], "text": license["text"], "used_by": used})
        inventory = {"schema_version": 1, "generator": f"cargo-about {VERSION}", "cargo_lock_sha256": hashlib.sha256(lock_bytes).hexdigest(), "targets": targets, "scope": "Locked workspace graph, including build dependencies and excluding dev dependencies; may include build-time components not linked into the app.", "rust_packages": [], "components": []}
        source_rows = []
        for package_id in sorted(selected, key=lambda key: (packages[key]["name"], packages[key]["version"])):
            package = packages[package_id]
            name, version = package["name"], package["version"]
            base = Path(package["manifest_path"]).parent
            key = (name, version)
            source_url = f"https://crates.io/api/v1/crates/{name}/{version}/download"
            if not package["source"].startswith("registry+"):
                raise RuntimeError(f"Review source delivery for non-registry dependency {name}")
            # Cargo's registry source and archive directories share a registry ID.
            archive = base.parent.parent.parent / "cache" / base.parent.name / f"{name}-{version}.crate"
            checked(archive, checksums[key])
            files = []
            original = notice_files(base)
            for source in original:
                target = dest / "licenses/rust" / f"{name}-{version}" / source.relative_to(base)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
                files.append(target.relative_to(dest).as_posix())
            extra = supplemental.get(key)
            if extra:
                vcs = json.loads((base / ".cargo_vcs_info.json").read_text())
                if vcs["git"]["sha1"] != extra["commit"]:
                    raise RuntimeError(f"Upstream notice commit mismatch: {name}")
                for item in extra["files"]:
                    source = checked(CONFIG / item["path"], item["sha256"])
                    target = dest / "licenses/rust" / f"{name}-{version}" / ("upstream-" + source.name)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
                    files.append(target.relative_to(dest).as_posix())
            if not files and not (extra and extra.get("review_note")):
                raise RuntimeError(f"Original license text missing for {name} {version}; add reviewed supplemental notices")
            metadata = dest / "licenses/rust" / f"{name}-{version}" / "UPSTREAM-PACKAGE.txt"
            metadata.parent.mkdir(parents=True, exist_ok=True)
            metadata.write_text(f"{name} {version}\n\nLicense declaration: {package['license']}\nAuthors (upstream package metadata): {', '.join(package['authors']) or 'not supplied'}\nRepository: {package['repository']}\nExact source: {source_url}\nSource SHA-256: {checksums[key]}\n\n" + (extra.get("review_note", "") if extra else "") + "\n")
            files.append(metadata.relative_to(dest).as_posix())
            record = {"name": name, "version": version, "authors": package["authors"], "declared_license": package["license"], "selected_licenses": sorted(selected[package_id]), "repository": package["repository"], "source_url": source_url, "source_sha256": checksums[key], "notice_files": files}
            if extra:
                record["supplemental_sources"] = extra["files"]
                if extra.get("review_note"):
                    record["notice_review"] = extra["review_note"]
            if "MPL-2.0" in selected[package_id]:
                # Verify the exact compiled source still matches its original
                # archive; local MPL edits require distributing corresponding edits.
                with tarfile.open(archive) as tar:
                    for member in tar:
                        if member.isfile():
                            relative = Path(member.name).relative_to(f"{name}-{version}")
                            if ".." in relative.parts:
                                raise RuntimeError(f"Unsafe source archive path for {name}")
                            if tar.extractfile(member).read() != (base / relative).read_bytes():
                                raise RuntimeError(f"MPL source modified locally: {name}/{relative}; package corresponding modified source")
                target = dest / "sources" / archive.name
                target.parent.mkdir(exist_ok=True)
                shutil.copyfile(archive, target)
                record["bundled_source"] = target.relative_to(dest).as_posix()
                source_rows.append(f"{name} {version}: {record['bundled_source']}\n  SHA-256: {checksums[key]}\n  Upstream: {source_url}\n")
            elif extra and extra.get("bundle_source"):
                target = dest / "sources" / archive.name
                target.parent.mkdir(exist_ok=True)
                shutil.copyfile(archive, target)
                record["bundled_source"] = target.relative_to(dest).as_posix()
            inventory["rust_packages"].append(record)

        def component(name, version, license, url, source, filename):
            target = dest / "licenses" / filename
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            inventory["components"].append({"name": name, "version": version, "license": license, "source_url": url, "notice_files": [target.relative_to(dest).as_posix()], "notice_sha256": digest(target)})

        shutil.copyfile(ROOT / "LICENSE", dest / "LICENSE.txt")
        component("JetBrains Mono", "bundled font", "OFL-1.1", "https://github.com/JetBrains/JetBrainsMono", ROOT / "apps/ui/fonts/OFL.txt", "JetBrainsMono-OFL.txt")
        transcription = ROOT / ".tools/transcription"
        whisper_archive = checked(transcription / f"whisper-{WHISPER_COMMIT}.tar.gz", WHISPER_SHA)
        whisper = transcription / f"whisper.cpp-{WHISPER_COMMIT}"
        component("whisper.cpp / ggml", "1.9.5", "MIT", f"https://github.com/ggml-org/whisper.cpp/tree/{WHISPER_COMMIT}", whisper / "LICENSE", "whisper.cpp-MIT.txt")
        component("Whisper Base multilingual model", "5359861c739e955e79d9a303bcbc70fb988958b1", "MIT", "https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1", checked(transcription / "LICENSE-whisper-model.txt", MODEL_LICENSE_SHA), "Whisper-model-MIT.txt")
        # The source archive retains additional native support-library copyrights
        # in miniaudio, stb_vorbis, nlohmann/json, ggml and llamafile source headers.
        # Keep it alongside the readable excerpts rather than flattening notices.
        native_source = dest / "sources" / whisper_archive.name
        native_source.parent.mkdir(exist_ok=True)
        shutil.copyfile(whisper_archive, native_source)
        inventory["components"][-2]["bundled_source"] = native_source.relative_to(dest).as_posix()
        inventory["components"][-2]["source_sha256"] = WHISPER_SHA
        native_excerpts = [
            ("miniaudio", "MIT-0 OR Unlicense", "examples/miniaudio.h", "This software is available as a choice of the following licenses."),
            ("stb_vorbis", "MIT OR Unlicense", "examples/stb_vorbis.c", "This software is available under 2 licenses -- choose whichever you prefer."),
            ("llamafile CPU matrix multiplication", "MIT", "ggml/src/ggml-cpu/llamafile/sgemm.cpp", None),
        ]
        with tarfile.open(whisper_archive) as tar:
            for name, license, path, start in native_excerpts:
                text = tar.extractfile(f"whisper.cpp-{WHISPER_COMMIT}/{path}").read().decode()
                text = text[text.index(start):] if start else text.split("\n\n//\n//", 1)[0]
                target = dest / "licenses" / (Path(path).name + "-NOTICE.txt")
                target.write_text(text)
                inventory["components"].append({"name": name, "version": "as bundled by whisper.cpp 1.9.5", "license": license, "source_url": f"https://github.com/ggml-org/whisper.cpp/blob/{WHISPER_COMMIT}/{path}", "notice_files": [target.relative_to(dest).as_posix()], "bundled_source": native_source.relative_to(dest).as_posix()})
            # json.hpp embeds SPDX notices throughout its amalgamated source.
            text = tar.extractfile(f"whisper.cpp-{WHISPER_COMMIT}/examples/json.hpp").read().decode()
            notices = sorted({line for line in text.splitlines() if "SPDX-FileCopyrightText:" in line or "SPDX-License-Identifier:" in line})
            target = dest / "licenses/nlohmann-json-NOTICE.txt"
            permission = "Permission is hereby granted" + (dest / "licenses/whisper.cpp-MIT.txt").read_text().split("Permission is hereby granted", 1)[1]
            target.write_text("nlohmann/json 3.11.2 (MIT)\n\n" + "\n".join(notices) + "\n\n" + permission)
            inventory["components"].append({"name": "nlohmann/json", "version": "3.11.2", "license": "MIT", "source_url": f"https://github.com/ggml-org/whisper.cpp/blob/{WHISPER_COMMIT}/examples/json.hpp", "notice_files": [target.relative_to(dest).as_posix()], "bundled_source": native_source.relative_to(dest).as_posix()})
        sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], env=env, text=True, cwd=ROOT).strip())
        rust_docs = sysroot / "share/doc/rust"
        component("Rust standard library", (ROOT / "rust-toolchain.toml").read_text().split('channel = "')[1].split('"')[0], "MIT OR Apache-2.0 (with notices for bundled support code)", "https://github.com/rust-lang/rust", rust_docs / "COPYRIGHT-library.html", "Rust-COPYRIGHT-library.html")
        shutil.copytree(rust_docs / "licenses", dest / "licenses/rust-runtime-licenses")
        (dest / "sources/README.txt").write_text("MPL-2.0 SOURCE AVAILABILITY\n\nThe exact, unchanged Source Code Form of the following dependencies is included in these .crate archives (gzip-compressed tar files). The covered files are made available under MPL-2.0, with their original notices retained. Extract using tar -xzf <filename>. You may obtain and modify this source under the MPL-2.0 terms included in the license notices. No Thesaurus-specific terms restrict those source rights.\n\n" + "\n".join(source_rows) + "\nADDITIONAL UPSTREAM SOURCE\n\nOther .crate archives preserve original source and notices for packages whose published archives omit a standalone license file. They retain the package-specific license identified in inventory.json; they are not all MPL-licensed. Upstream package metadata and recovered repository notices are included under licenses/rust.\n\nNATIVE SUPPORT CODE\n\nThe pinned whisper.cpp source archive also preserves original notices and source for ggml and its bundled support code, including miniaudio, stb_vorbis, nlohmann/json and llamafile. The source archive includes upstream examples and tools that are not all compiled into Thesaurus.\n")
        write_json(dest / "rust-licenses.json", {"licenses": licenses})
        write_json(dest / "inventory.json", inventory)
        render_index(dest, inventory, licenses)
        manifest = {p.relative_to(dest).as_posix(): digest(p) for p in sorted(dest.rglob("*")) if p.is_file()}
        write_json(dest / "checksums.json", manifest)
        output = parent / "notices"
        if output.exists():
            shutil.rmtree(output)
        shutil.copytree(dest, output)
        print(f"Third-party notices: {len(inventory['rust_packages'])} Rust packages, {len(source_rows)} MPL source archives, {len(inventory['components'])} native/assets/runtime entries -> {output}")


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def render_index(dest, inventory, licenses):
    esc = html.escape
    def link(path, label):
        return f'<a href="{esc(path, quote=True)}">{esc(label)}</a>'
    parts = ['<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Thesaurus — Licenses and notices</title><style>body{font:16px/1.6 system-ui,sans-serif;max-width:80rem;margin:2rem auto;padding:0 1.5rem;color:#202026;background:#fff}a{color:#233e9b}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:13px/1.5 ui-monospace,monospace;background:#f5f5f7;padding:1rem}td,th{text-align:left;vertical-align:top;padding:.5rem;border-bottom:1px solid #ddd}table{border-collapse:collapse;width:100%}summary{cursor:pointer}h2{margin-top:3rem}</style><h1>Thesaurus: licenses and third-party notices</h1><p>Original Thesaurus code is distributed under the ', link('LICENSE.txt','MIT license'), '. Third-party components retain the licenses and copyright notices below.</p><p>Exact source archives for MPL-2.0 components are included with this app. See ', link('sources/README.txt','source availability and extraction instructions'), '. Covered source remains available under MPL-2.0.</p><p>', link('inventory.json','Machine-readable component inventory'), ' · ', link('rust-licenses.json','Selected Rust license texts'), ' · ', link('checksums.json','Notice bundle checksums'), '</p><h2>Assets, native support code and runtime</h2><table><tr><th>Component</th><th>License and original notices</th><th>Source</th></tr>']
    for c in inventory['components']:
        files=' · '.join(link(f,Path(f).name) for f in c['notice_files'])
        source=link(c['source_url'],'Upstream source')
        if c.get('bundled_source'): source+=' · '+link(c['bundled_source'],'Included source archive')
        parts.append(f'<tr><td>{esc(c["name"])}<br>{esc(c["version"])}</td><td>{esc(c["license"])}<br>{files}</td><td>{source}</td></tr>')
    parts.append('</table><h2>Rust dependency inventory</h2><p>'+esc(inventory['scope'])+'</p><table><tr><th>Package</th><th>Selected license(s) and original notices</th><th>Source</th></tr>')
    for c in inventory['rust_packages']:
        files='<br>'.join(link(f,Path(f).name) for f in c['notice_files'])
        source=link(c['source_url'],'Exact published source')
        if c.get('repository'): source+=' · '+link(c['repository'],'Repository')
        if c.get('bundled_source'): source+='<br>'+link(c['bundled_source'],'Included exact source archive')
        parts.append(f'<tr><td>{esc(c["name"])} {esc(c["version"])}</td><td>{esc(" AND ".join(c["selected_licenses"]))}<br>{files}</td><td>{source}</td></tr>')
    parts.append('</table><h2>Selected license texts</h2>')
    for l in licenses:
        used=', '.join(f'{p["name"]} {p["version"]}' for p in l['used_by'])
        parts.append(f'<details><summary>{esc(l["name"])} — {esc(used)}</summary><pre>{esc(l["text"])}</pre></details>')
    parts.append('</html>')
    (dest/'index.html').write_text(''.join(parts))


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, OSError, KeyError) as error:
        sys.exit(f"Notice generation failed: {error}")
