#!/usr/bin/env python3
"""Validate the assembled Apple Silicon app without opening a user's workspace."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import os
import subprocess
import sys
from html.parser import HTMLParser
import re
from urllib.parse import unquote, urlsplit

BASE_MODEL_SHA256 = '60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe'


def run(command):
    result = subprocess.run([str(arg) for arg in command], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{Path(command[0]).name} failed: {result.stderr.strip()}')
    return result.stdout.strip()


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def contained_file(root, relative):
    path = root/relative
    if Path(relative).is_absolute() or '..' in Path(relative).parts:
        raise RuntimeError(f'Invalid resource path: {relative}')
    if not path.is_file() or any(part.is_symlink() for part in [path, *path.parents] if part != root.parent):
        raise RuntimeError(f'Missing or symlinked resource: {relative}')
    path.resolve().relative_to(root.resolve())
    return path


class Assets(HTMLParser):
    def __init__(self):
        super().__init__()
        self.paths = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'script' and attrs.get('src'):
            self.paths.append(attrs['src'])
        if tag == 'link' and attrs.get('href'):
            self.paths.append(attrs['href'])


def verify_web(root):
    parser = Assets()
    html = (root/'index.html').read_text()
    parser.feed(html)
    references = [(root, path) for path in parser.paths]
    overrides = re.findall(r'''\bmodule_or_path\s*:\s*["']([^"']+)["']''', html)
    references.extend((root, path) for path in overrides)
    # Validate actual imports and URL constructors. wasm-bindgen also emits
    # virtual import-object keys ending in .js; those are not fetched files.
    for script in root.rglob('*.js'):
        text = script.read_text()
        if script.parent == root and overrides:
            # Trunk explicitly supplies the hashed module path in its bootstrap.
            # wasm-bindgen retains an unused, unhashed default behind this guard.
            text = re.sub(r'''if\s*\(\s*module_or_path\s*===\s*undefined\s*\)\s*\{\s*module_or_path\s*=\s*new\s+URL\([^;]+;\s*\}''', '', text)
        for pattern in [r'''\bfrom\s*["']([^"']+)["']''',
                        r'''\bimport\s*(?:\(\s*)?["']([^"']+)["']''',
                        r'''\bnew\s+URL\(\s*["']([^"']+)["']\s*,\s*import\.meta\.url''']:
            references.extend((script.parent, path) for path in re.findall(pattern, text))
    for css in root.rglob('*.css'):
        references.extend((css.parent, path) for path in re.findall(r'''url\(["']?([^\)"']+)["']?\)''', css.read_text()))
    for base, relative in references:
        url = urlsplit(relative)
        if url.scheme == 'data':
            continue
        if url.scheme or url.netloc:
            raise RuntimeError(f'Bundled web assets must be local: {relative}')
        path = unquote(url.path)
        local = (root/path.lstrip('/')) if path.startswith('/') else (base/path)
        try:
            normalized = Path(os.path.normpath(local)).relative_to(root)
        except ValueError:
            raise RuntimeError(f'Web asset escapes the bundle: {relative}') from None
        contained_file(root, str(normalized))
    if not list(root.glob('*.wasm')) or not list(root.glob('*.js')):
        raise RuntimeError('Bundled WebAssembly or JavaScript is missing')


def verify_notices(root):
    expected = json.loads(contained_file(root, 'checksums.json').read_text())
    if not isinstance(expected, dict) or not expected:
        raise RuntimeError('Notices checksum manifest is empty or malformed')
    for relative, checksum in expected.items():
        if digest(contained_file(root, relative)) != checksum:
            raise RuntimeError(f'Notice or corresponding source checksum mismatch: {relative}')


def verify(app):
    if app.is_symlink() or not app.is_dir() or app.suffix != '.app':
        raise RuntimeError('Select an existing .app directory, not a symlink')
    app = app.resolve()
    info = plistlib.loads((app/'Contents/Info.plist').read_bytes())
    resources = app/'Contents/Resources/resources'
    executables = [app/'Contents/MacOS'/info['CFBundleExecutable'],
                   resources/'thesaurus', resources/'thesaurus-audio',
                   resources/'thesaurus-ocr', resources/'transcription/whisper-cli']
    run(['/usr/bin/codesign', '--verify', '--deep', '--strict', app])
    hashes = {}
    libraries = {}
    for executable in executables:
        if not executable.is_file() or executable.is_symlink():
            raise RuntimeError(f'Missing executable: {executable.relative_to(app)}')
        if not os.access(executable, os.X_OK):
            raise RuntimeError(f'Packaged helper is not executable: {executable.name}')
        if run(['/usr/bin/lipo', '-archs', executable]) != 'arm64':
            raise RuntimeError(f'Expected an Apple Silicon executable: {executable.name}')
        run(['/usr/bin/codesign', '--verify', '--strict', executable])
        linked = [line.strip().split(' (')[0] for line in run(['/usr/bin/otool', '-L', executable]).splitlines()[1:]]
        # System Swift libraries are provided by macOS. Development toolchain,
        # Homebrew and checkout-local paths must never escape into an installer.
        for library in linked:
            if not library.startswith(('/System/Library/', '/usr/lib/')):
                raise RuntimeError(f'Non-system dynamic library in {executable.name}: {library}')
        relative = str(executable.relative_to(app))
        libraries[relative] = linked
        hashes[relative] = digest(executable)
    required = ['web/index.html', 'notices/index.html', 'notices/inventory.json',
                'notices/LICENSE.txt', 'transcription/LICENSE-whisper.cpp.txt',
                'transcription/LICENSE-whisper-model.txt']
    for relative in required:
        path = resources/relative
        if not path.is_file() or not path.stat().st_size:
            raise RuntimeError(f'Missing bundled resource: {relative}')
        hashes['Contents/Resources/resources/'+relative] = digest(path)
    verify_web(resources/'web')
    verify_notices(resources/'notices')
    model = resources/'transcription/ggml-base.bin'
    if not model.is_file() or digest(model) != BASE_MODEL_SHA256:
        raise RuntimeError('Bundled Base transcription model is missing or has the wrong hash')
    hashes['Contents/Resources/resources/transcription/ggml-base.bin'] = BASE_MODEL_SHA256
    return {'bundle_id': info['CFBundleIdentifier'], 'version': info['CFBundleShortVersionString'],
            'architecture': 'arm64', 'minimum_macos': info.get('LSMinimumSystemVersion'),
            'signature_valid': True, 'system_dynamic_libraries_only': True,
            'executables_and_resources_sha256': hashes, 'dynamic_libraries': libraries}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    try:
        report = verify(args.app)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2)+'\n')
        print(f'Verified {report["version"]} arm64 app: signatures, helpers, model, web assets and notices')
    except (OSError, RuntimeError, KeyError, ValueError) as error:
        print(f'App verification failed: {error}', file=sys.stderr)
        sys.exit(1)
