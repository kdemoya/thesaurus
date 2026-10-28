#!/usr/bin/env python3
"""Check installer rejection paths without real signing, uploads or app launches."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), SCRIPTS/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


verifier = load('verify-macos')
packager = load('package-macos')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(root, relative, content=b'fixture'):
    path = root/relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if isinstance(content, bytes) else content.encode())
    return path


def web_fixture(root):
    write(root, 'index.html', '<link rel="stylesheet" href="/style.css"><script src="/app.js"></script>')
    write(root, 'app.js', 'const model = new URL("app_bg.wasm", import.meta.url);')
    write(root, 'app_bg.wasm')
    write(root, 'style.css', '@font-face { font-family: fixture; src: url("/fixture.woff2"); }')
    write(root, 'fixture.woff2')


def notices_fixture(root):
    for relative in ('index.html', 'inventory.json', 'LICENSE.txt',
                     'licenses/MPL-2.0.txt', 'sources/fixture-1.0.0.crate'):
        write(root, relative)
    manifest = {str(path.relative_to(root)): digest(path) for path in root.rglob('*') if path.is_file()}
    write(root, 'checksums.json', json.dumps(manifest))


def app_fixture(root):
    app = root/'Thesaurus.app'
    write(app, 'Contents/Info.plist', plistlib.dumps({
        'CFBundleExecutable': 'thesaurus-desktop',
        'CFBundleIdentifier': 'dev.thesaurus.desktop',
        'CFBundleShortVersionString': '0.1.0',
        'LSMinimumSystemVersion': '12.0',
    }))
    for relative in ('Contents/MacOS/thesaurus-desktop',
                     'Contents/Resources/resources/thesaurus',
                     'Contents/Resources/resources/thesaurus-audio',
                     'Contents/Resources/resources/thesaurus-ocr',
                     'Contents/Resources/resources/transcription/whisper-cli'):
        write(app, relative).chmod(0o755)
    resources = app/'Contents/Resources/resources'
    web_fixture(resources/'web')
    notices_fixture(resources/'notices')
    write(resources, 'transcription/LICENSE-whisper.cpp.txt')
    write(resources, 'transcription/LICENSE-whisper-model.txt')
    model = write(resources, 'transcription/ggml-base.bin')
    return app, digest(model)


def inspect_binary(command):
    name = Path(command[0]).name
    if name == 'lipo':
        return 'arm64'
    if name == 'otool':
        return f'{command[-1]}:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)'
    if name == 'codesign':
        return ''
    raise AssertionError(f'Unexpected external tool: {name}')


class WebAssetTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()/'web'
        web_fixture(self.root)

    def test_complete_web_fixture_passes(self):
        verifier.verify_web(self.root)

    def test_wasm_bindgen_virtual_module_key_is_not_a_fetched_file(self):
        with (self.root/'app.js').open('a') as script:
            script.write('\nconst imports = {"./virtual_bg.js": {}};')
        verifier.verify_web(self.root)

    def test_trunk_hashed_wasm_override_replaces_unused_bindgen_default(self):
        with (self.root/'index.html').open('a') as html:
            html.write('<script>init({module_or_path: "/app_bg.wasm"});</script>')
        with (self.root/'app.js').open('a') as script:
            script.write('\nif (module_or_path === undefined) { module_or_path = new URL("unhashed_bg.wasm", import.meta.url); }')
        verifier.verify_web(self.root)
        (self.root/'app_bg.wasm').unlink()
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)

    def test_missing_referenced_javascript_is_rejected_even_with_another_script(self):
        (self.root/'app.js').unlink()
        write(self.root, 'unrelated.js')
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)

    def test_missing_referenced_wasm_is_rejected_even_with_another_module(self):
        (self.root/'app_bg.wasm').unlink()
        write(self.root, 'unrelated.wasm')
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)

    def test_missing_css_font_is_rejected(self):
        (self.root/'fixture.woff2').unlink()
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)

    def test_external_asset_is_rejected_without_network_access(self):
        write(self.root, 'index.html', '<script src="https://example.invalid/app.js"></script>')
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)

    def test_symlinked_asset_is_rejected(self):
        (self.root/'app.js').unlink()
        outside = write(self.root.parent, 'outside.js', 'const model = "app_bg.wasm";')
        (self.root/'app.js').symlink_to(outside)
        with self.assertRaises(RuntimeError):
            verifier.verify_web(self.root)


class NoticeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()/'notices'
        notices_fixture(self.root)

    def test_complete_notice_fixture_passes(self):
        verifier.verify_notices(self.root)

    def test_changed_notice_is_rejected(self):
        write(self.root, 'licenses/MPL-2.0.txt', 'changed text')
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)

    def test_missing_corresponding_source_is_rejected(self):
        (self.root/'sources/fixture-1.0.0.crate').unlink()
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)

    def test_source_path_traversal_and_absolute_paths_are_rejected(self):
        outside = write(self.root.parent, 'outside.crate')
        for relative in ('../outside.crate', str(outside)):
            with self.subTest(relative=relative):
                write(self.root, 'checksums.json', json.dumps({relative: digest(outside)}))
                with self.assertRaises(RuntimeError):
                    verifier.verify_notices(self.root)

    def test_source_symlink_is_rejected_even_if_contents_match_checksum(self):
        path = self.root/'sources/fixture-1.0.0.crate'
        path.unlink()
        path.symlink_to(write(self.root.parent, 'outside.crate'))
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)

    def test_source_directory_symlink_is_rejected(self):
        path = self.root/'sources/fixture-1.0.0.crate'
        path.unlink()
        path.parent.rmdir()
        outside = self.root.parent/'outside-sources'
        write(outside, path.name)
        path.parent.symlink_to(outside, target_is_directory=True)
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)

    def test_missing_or_empty_checksum_manifest_is_rejected(self):
        (self.root/'checksums.json').unlink()
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)
        write(self.root, 'checksums.json', '{}')
        with self.assertRaises(RuntimeError):
            verifier.verify_notices(self.root)


class ExecutableTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.app, model_hash = app_fixture(Path(self.temp.name).resolve())
        self.model_patch = patch.object(verifier, 'BASE_MODEL_SHA256', model_hash)
        self.model_patch.start()
        self.addCleanup(self.model_patch.stop)
        self.tools_patch = patch.object(verifier, 'run', side_effect=inspect_binary)
        self.tools_patch.start()
        self.addCleanup(self.tools_patch.stop)

    def test_complete_self_contained_app_passes(self):
        report = verifier.verify(self.app)
        self.assertTrue(report['system_dynamic_libraries_only'])
        self.assertEqual(report['architecture'], 'arm64')

    def test_homebrew_and_rpath_links_in_nested_helper_are_rejected(self):
        for library in ('/opt/homebrew/opt/fixture/lib/libfixture.dylib', '@rpath/libfixture.dylib'):
            def inspect(command):
                if Path(command[0]).name == 'otool' and Path(command[-1]).name == 'whisper-cli':
                    return f'{command[-1]}:\n\t{library} (compatibility version 1.0.0, current version 1.0.0)'
                return inspect_binary(command)
            with self.subTest(library=library), patch.object(verifier, 'run', side_effect=inspect):
                with self.assertRaises(RuntimeError):
                    verifier.verify(self.app)

    def test_nonexecutable_helper_is_rejected(self):
        (self.app/'Contents/Resources/resources/thesaurus-ocr').chmod(0o644)
        with self.assertRaises(RuntimeError):
            verifier.verify(self.app)

    def test_intel_helper_is_rejected(self):
        def inspect(command):
            if Path(command[0]).name == 'lipo' and Path(command[-1]).name == 'thesaurus-ocr':
                return 'x86_64'
            return inspect_binary(command)
        with patch.object(verifier, 'run', side_effect=inspect):
            with self.assertRaises(RuntimeError):
                verifier.verify(self.app)

    def test_changed_model_is_rejected(self):
        write(self.app, 'Contents/Resources/resources/transcription/ggml-base.bin', 'changed model')
        with self.assertRaises(RuntimeError):
            verifier.verify(self.app)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.root_patch = patch.object(packager, 'ROOT', self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        for name, value in (('system', 'Darwin'), ('machine', 'arm64')):
            tool = patch.object(packager.platform, name, return_value=value)
            tool.start()
            self.addCleanup(tool.stop)

    def test_distribution_cannot_package_an_existing_unproven_build(self):
        args = argparse.Namespace(distribution=True, notary_profile='existing-profile', skip_build=True)
        with patch.object(packager, 'run') as run, patch.object(packager, 'assemble') as assemble:
            with self.assertRaisesRegex(RuntimeError, 'cannot use --skip-build'):
                packager.package(args)
            run.assert_not_called()
            assemble.assert_not_called()

    def test_local_mode_cannot_accidentally_use_notary_credentials(self):
        args = argparse.Namespace(distribution=False, notary_profile='existing-profile', skip_build=True)
        with patch.object(packager, 'run') as run:
            with self.assertRaises(RuntimeError):
                packager.package(args)
            run.assert_not_called()

    def test_failure_removes_partial_package_and_preserves_previous_success(self):
        previous = self.root/'target/macos-alpha/local/previous-success'
        write(previous, 'release.json', '{"fixture":"existing successful release"}')
        before = (previous/'release.json').read_bytes()
        def fail_assembly(app, output, args):
            write(output, 'partial.dmg')
            write(output, 'release.json', '{"incomplete":true}')
            raise subprocess.CalledProcessError(1, ['hdiutil', 'create'])
        args = argparse.Namespace(distribution=False, notary_profile=None, skip_build=True)
        with patch.object(packager, 'run') as run, patch.object(packager, 'assemble', side_effect=fail_assembly):
            with self.assertRaises(subprocess.CalledProcessError):
                packager.package(args)
            run.assert_not_called()
        self.assertEqual((previous/'release.json').read_bytes(), before)
        self.assertEqual(list(previous.parent.iterdir()), [previous])


if __name__ == '__main__':
    unittest.main()
