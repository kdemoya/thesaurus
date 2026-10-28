#!/usr/bin/env python3
"""Submit a Developer ID artifact using an existing notarytool Keychain profile.

This command uploads the selected artifact to Apple. Run it explicitly after
reviewing the build. It never creates credentials, certificates or keychains.
Notarize and staple the app before creating, signing and notarizing its DMG.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(command):
    result = subprocess.run([str(value) for value in command], capture_output=True, text=True)
    if result.returncode:
        # Keychain profile names are not secrets. Credentials stay in Keychain
        # and never appear in this command's arguments, reports, or output.
        raise RuntimeError(f'{Path(command[0]).name} failed: {result.stderr.strip() or result.stdout.strip()}')
    return result.stdout.strip()


def submit(artifact, profile, keychain, timeout):
    command = ['/usr/bin/xcrun', 'notarytool', 'submit', artifact,
               '--keychain-profile', profile, '--wait', '--timeout', timeout,
               '--output-format', 'json']
    if keychain:
        command += ['--keychain', keychain]
    response = json.loads(run(command))
    if not isinstance(response, dict) or not response.get('id') or not response.get('status'):
        raise RuntimeError('Apple returned an incomplete notarization response; do not distribute the artifact')
    if response['status'] != 'Accepted':
        raise RuntimeError(f'Notarization {response["id"]} was {response["status"]}; inspect it with xcrun notarytool log and do not distribute the artifact')
    return response


def notarize(artifact, profile, keychain=None, timeout='30m'):
    if artifact.is_symlink():
        raise RuntimeError('Select an actual artifact, not a symlink')
    is_app = artifact.suffix == '.app' and artifact.is_dir()
    is_dmg = artifact.suffix.lower() == '.dmg' and artifact.is_file()
    if not is_app and not is_dmg:
        raise RuntimeError('Artifact must be an existing .app directory or regular .dmg file')
    artifact = artifact.resolve()
    kind = '--app' if is_app else '--dmg'
    run([sys.executable, ROOT/'scripts/sign-macos.py', '--distribution', '--verify-only', kind, artifact])
    if is_app:
        # notarytool accepts ZIP files rather than app directories. Keeping the
        # parent preserves the app bundle while the temporary upload is removed.
        with tempfile.TemporaryDirectory(prefix='thesaurus-notary-') as tmp:
            archive = Path(tmp)/'Thesaurus.zip'
            run(['/usr/bin/ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', artifact, archive])
            response = submit(archive, profile, keychain, timeout)
    else:
        response = submit(artifact, profile, keychain, timeout)
    run(['/usr/bin/xcrun', 'stapler', 'staple', artifact])
    run(['/usr/bin/xcrun', 'stapler', 'validate', artifact])
    # Stapling is the final mutation. Assess the exact artifact users receive.
    if is_app:
        run(['/usr/sbin/spctl', '--assess', '--type', 'execute', '--verbose=2', artifact])
    else:
        run(['/usr/sbin/spctl', '--assess', '--type', 'open', '--context', 'context:primary-signature', '--verbose=2', artifact])
    return {'artifact': str(artifact), 'submission_id': response['id'],
            'status': response['status'], 'stapled': True,
            'staple_valid': True, 'gatekeeper_assessment_passed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    artifact = parser.add_mutually_exclusive_group(required=True)
    artifact.add_argument('--app', type=Path)
    artifact.add_argument('--dmg', type=Path)
    parser.add_argument('--profile', default=os.environ.get('THESAURUS_NOTARY_PROFILE'),
                        help='Existing notarytool Keychain profile (or THESAURUS_NOTARY_PROFILE)')
    parser.add_argument('--keychain', default=os.environ.get('THESAURUS_NOTARY_KEYCHAIN'),
                        help='Keychain containing the notarytool profile, if outside the default search list')
    parser.add_argument('--timeout', default='30m', help='Maximum notarytool wait, such as 30m; timeout leaves the artifact unapproved')
    parser.add_argument('--report', type=Path, help='Write a public notarization receipt after all validation succeeds')
    args = parser.parse_args()
    if not args.profile or not args.profile.strip():
        parser.error('Set --profile or THESAURUS_NOTARY_PROFILE to an existing notarytool Keychain profile')
    result = notarize(args.app or args.dmg, args.profile, args.keychain, args.timeout)
    if args.report:
        if args.report.is_symlink():
            raise RuntimeError('Report path must not be a symlink')
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, indent=2)+'\n')
    print(f'Notarized, stapled and assessed {result["artifact"]} ({result["submission_id"]})')


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        print(f'Notarization failed: {error}', file=sys.stderr)
        sys.exit(1)
