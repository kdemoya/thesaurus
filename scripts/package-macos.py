#!/usr/bin/env python3
"""Build a local alpha DMG, or explicitly sign and notarize a distribution build.

Default operation stays local. --distribution requires an existing Developer ID
identity and an existing notarytool profile, and uploads artifacts to Apple.
Nothing is published to GitHub or a download host.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(command, capture=False):
    result = subprocess.run([str(arg) for arg in command], cwd=ROOT, check=True,
                            capture_output=capture, text=True)
    return result.stdout.strip() if capture else None


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def package(args):
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        raise RuntimeError('The initial installer targets an Apple Silicon Mac build host')
    signing = '--distribution' if args.distribution else '--local'
    if args.distribution:
        if args.skip_build:
            raise RuntimeError('--distribution cannot use --skip-build; binaries must be built from the reviewed commit')
        if not args.notary_profile:
            raise RuntimeError('--distribution requires --notary-profile; local signing cannot substitute')
        if run(['git', 'status', '--porcelain'], capture=True):
            raise RuntimeError('Distribution builds require a clean, reviewed source commit')
        run([sys.executable, ROOT/'scripts/sign-macos.py', '--distribution', '--check-identity'])
    elif args.notary_profile:
        raise RuntimeError('--notary-profile requires explicit --distribution')
    if not args.skip_build:
        run([ROOT/'scripts/build-desktop.sh', '--release', signing])
    app = ROOT/'target/release/bundle/macos/Thesaurus.app'
    base = ROOT/'target/macos-alpha'/('distribution' if args.distribution else 'local')
    base.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.building-', dir=base) as pending:
        output = Path(pending)
        assemble(app, output, args)
        final = base/datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
        output.rename(final)
    print(f'Alpha artifacts: {final}')
    if not args.distribution:
        print('Local review build. Developer ID signing, notarization and clean-machine acceptance remain pending.')


def assemble(app, output, args):
    run([sys.executable, ROOT/'scripts/verify-macos.py', '--app', app,
         '--report', output/'app-verification.json'])
    report = json.loads((output/'app-verification.json').read_text())
    run([sys.executable, ROOT/'scripts/smoke-macos.py', '--app', app,
         '--report', output/'packaged-service-smoke.json'])
    if json.loads((output/'packaged-service-smoke.json').read_text()).get('status') != 'passed':
        raise RuntimeError('Packaged service smoke did not pass')
    stem = f'Thesaurus_{report["version"]}_arm64_'+('alpha' if args.distribution else 'local-alpha')
    if args.distribution:
        run([sys.executable, ROOT/'scripts/notarize-macos.py', '--app', app,
             '--profile', args.notary_profile, '--report', output/'app-notarization.json'])
        # Stapling changes the bundle. Record the final app, not its predecessor.
        run([sys.executable, ROOT/'scripts/verify-macos.py', '--app', app,
             '--report', output/'app-verification.json'])
    instructions = (ROOT/'docs/macos-alpha-install.md').read_text()
    (output/'INSTALL.md').write_text(instructions)
    dmg = output/(stem+'.dmg')
    with tempfile.TemporaryDirectory(prefix='dmg-stage-', dir=output) as tmp:
        staging = Path(tmp)
        run(['/usr/bin/ditto', app, staging/'Thesaurus.app'])
        (staging/'Applications').symlink_to('/Applications', target_is_directory=True)
        (staging/'INSTALL.md').write_text(instructions)
        shutil.copy2(ROOT/'LICENSE', staging/'LICENSE.txt')
        shutil.copytree(app/'Contents/Resources/resources/notices', staging/'Notices')
        run(['/usr/bin/hdiutil', 'create', '-volname', 'Thesaurus Alpha',
             '-srcfolder', staging, '-format', 'UDZO', '-ov', dmg])
    sign_args = ['--distribution'] if args.distribution else ['--allow-ad-hoc']
    run([sys.executable, ROOT/'scripts/sign-macos.py', '--dmg', dmg, *sign_args])
    if args.distribution:
        run([sys.executable, ROOT/'scripts/notarize-macos.py', '--dmg', dmg,
             '--profile', args.notary_profile, '--report', output/'dmg-notarization.json'])
    run(['/usr/bin/hdiutil', 'verify', dmg])
    archive = output/(stem+'.app.zip')
    if archive.exists():
        archive.unlink()
    run(['/usr/bin/ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', app, archive])
    manifest = {'version': report['version'], 'channel': 'alpha' if args.distribution else 'local-alpha',
                'architecture': 'arm64', 'created_utc': datetime.now(timezone.utc).isoformat(),
                'source_commit': run(['git', 'rev-parse', 'HEAD'], capture=True),
                'source_has_local_changes': bool(run(['git', 'status', '--porcelain'], capture=True)),
                'build_skipped': args.skip_build,
                'packaged_service_smoke_passed': True,
                'developer_id_signed': args.distribution, 'notarized_and_stapled': args.distribution,
                'public_distribution_ready': False,
                'publication_approved': False,
                'fresh_machine_acceptance_complete': False,
                'artifacts': {path.name: {'bytes': path.stat().st_size, 'sha256': sha256(path)}
                              for path in [dmg, archive]}}
    (output/'release.json').write_text(json.dumps(manifest, indent=2)+'\n')
    files = sorted(path for path in output.iterdir() if path.is_file() and path.name != 'SHA256SUMS')
    (output/'SHA256SUMS').write_text(''.join(f'{sha256(path)}  {path.name}\n' for path in files))



if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--distribution', action='store_true', help='Explicitly sign and upload to Apple for notarization')
    parser.add_argument('--notary-profile', help='Existing notarytool Keychain profile; credentials stay in Keychain')
    parser.add_argument('--skip-build', action='store_true', help='Package an existing, verified release app instead of rebuilding')
    args = parser.parse_args()
    try:
        package(args)
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'Packaging failed: {error}', file=sys.stderr)
        sys.exit(1)
