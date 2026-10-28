#!/usr/bin/env python3
"""Sign Thesaurus code inside out for local builds or Developer ID distribution.

Creating an identity is an explicit operation. Builds never create certificates,
change certificate trust, or reset macOS privacy permissions.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import secrets
import shlex
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / '.local' / 'signing'
CONFIG = STATE / 'identity.json'
AUDIO_ENTITLEMENTS = ROOT/'apps/desktop/src-tauri/AudioEntitlements.plist'
AUDIO_INPUT = 'com.apple.security.device.audio-input'


def run(args, sensitive=False):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if result.returncode:
        detail = '' if sensitive else ': ' + result.stderr.strip()
        raise RuntimeError(f'{Path(args[0]).name} {args[1]} failed{detail}')
    return result.stdout.strip()


def private_file(path, body):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as f:
        f.write(body)
        f.flush()
        os.fsync(f.fileno())


def create_local():
    if CONFIG.exists():
        print('A persistent local identity already exists; reusing it preserves code identity.')
        return
    if STATE.is_symlink():
        raise RuntimeError('Signing directory must not be a symlink')
    STATE.mkdir(parents=True, mode=0o700, exist_ok=True)
    STATE.chmod(0o700)
    keychain = STATE / 'ThesaurusDevelopment.keychain-db'
    if keychain.exists() or keychain.with_suffix('').exists():
        raise RuntimeError('Partial signing setup exists; preserve it and inspect before proceeding')
    password_path = STATE / 'keychain-password'
    password = secrets.token_hex(32)
    private_file(password_path, password + '\n')
    original_search = shlex.split(run(['/usr/bin/security', 'list-keychains', '-d', 'user']))
    with tempfile.TemporaryDirectory(prefix='identity-', dir=STATE) as tmp:
        tmp = Path(tmp)
        key, cert, p12 = tmp/'key.pem', tmp/'certificate.pem', tmp/'identity.p12'
        spec = tmp/'openssl.cnf'
        spec.write_text('''[req]
prompt = no
distinguished_name = subject
x509_extensions = signing
[subject]
CN = Thesaurus Local Development
[signing]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
''')
        run(['/usr/bin/openssl', 'req', '-new', '-x509', '-newkey', 'rsa:3072', '-nodes',
             '-sha256', '-days', '3650', '-config', spec, '-keyout', key, '-out', cert], True)
        key.chmod(0o600)
        run(['/usr/bin/openssl', 'pkcs12', '-export', '-inkey', key, '-in', cert, '-out', p12,
             '-passout', 'file:' + str(password_path)], True)
        try:
            run(['/usr/bin/security', 'create-keychain', '-p', password, keychain], True)
        finally:
            # A private build keychain is addressed explicitly, never made the
            # default or retained in the user's general search list.
            run(['/usr/bin/security', 'list-keychains', '-d', 'user', '-s', *original_search], True)
        keychain.chmod(0o600)
        run(['/usr/bin/security', 'unlock-keychain', '-p', password, keychain], True)
        run(['/usr/bin/security', 'import', p12, '-k', keychain, '-f', 'pkcs12', '-P', password,
             '-x', '-T', '/usr/bin/codesign'], True)
        fingerprint = run(['/usr/bin/openssl', 'x509', '-in', cert, '-noout', '-fingerprint', '-sha1']).split('=')[-1].replace(':','').upper()
        if not re.fullmatch(r'[A-F0-9]{40}', fingerprint):
            raise RuntimeError('Could not identify the signing certificate')
        private_file(STATE/'certificate.pem', cert.read_text())
        private_file(CONFIG, json.dumps({'identity':fingerprint, 'keychain':str(keychain)}, indent=2)+'\n')
    print('Created a persistent local code-signing identity in a private project keychain.')
    print('No system certificate trust or privacy permission was changed.')


def distribution_identity():
    """Select an explicitly requested, valid Developer ID Application identity.

    Only public identity metadata is read. Distribution never reads the project's
    local signing configuration, unlocks a keychain, or falls back to ad hoc.
    """
    requested = os.environ.get('APPLE_SIGNING_IDENTITY', '').strip()
    if not requested or requested == '-':
        raise RuntimeError('Distribution requires APPLE_SIGNING_IDENTITY naming an existing Developer ID Application identity')
    keychain = os.environ.get('THESAURUS_SIGNING_KEYCHAIN') or None
    command = ['/usr/bin/security', 'find-identity', '-v', '-p', 'codesigning']
    if keychain:
        command.append(keychain)
    matches = []
    for fingerprint, name in re.findall(r'^\s*\d+\)\s+([A-Fa-f0-9]{40})\s+"([^"]+)"\s*$', run(command), re.MULTILINE):
        if requested.casefold() in (fingerprint.casefold(), name.casefold()):
            matches.append((fingerprint.upper(), name))
    if len(matches) != 1 or not re.fullmatch(r'Developer ID Application: .+ \([A-Z0-9]{10}\)', matches[0][1]):
        raise RuntimeError('APPLE_SIGNING_IDENTITY must match exactly one available Developer ID Application identity (name or SHA-1 fingerprint)')
    return matches[0][0], keychain, None


def identity(allow_ad_hoc, distribution=False):
    if distribution:
        return distribution_identity()
    explicit = os.environ.get('APPLE_SIGNING_IDENTITY')
    if explicit and explicit != '-':
        return explicit, os.environ.get('THESAURUS_SIGNING_KEYCHAIN'), None
    if not explicit and CONFIG.is_file():
        c = json.loads(CONFIG.read_text())
        if not re.fullmatch(r'[A-Fa-f0-9]{40}', c['identity']):
            raise RuntimeError('Invalid local signing identity')
        keychain = STATE/'ThesaurusDevelopment.keychain-db'
        if Path(c['keychain']) != keychain or not keychain.is_file() or keychain.is_symlink():
            raise RuntimeError('Persistent signing keychain is missing or invalid; do not replace its identity')
        return c['identity'], str(keychain), (STATE/'keychain-password').read_text().strip()
    if not allow_ad_hoc:
        raise RuntimeError('A persistent signing identity is required. Set APPLE_SIGNING_IDENTITY or explicitly create a local identity with --create-local-identity.')
    print('WARNING: ad hoc signing changes code identity on rebuild; macOS privacy grants may be lost.', file=sys.stderr)
    return '-', None, None


def audio_entitlements_file():
    if AUDIO_ENTITLEMENTS.is_symlink() or not AUDIO_ENTITLEMENTS.is_file():
        raise RuntimeError('The checked-in audio entitlements file is missing or symlinked')
    if plistlib.loads(AUDIO_ENTITLEMENTS.read_bytes()) != {AUDIO_INPUT: True}:
        raise RuntimeError('Audio entitlements must contain only the required audio-input permission')
    return AUDIO_ENTITLEMENTS


def signing_command(signer, keychain, distribution, executable=True, entitlements=None):
    command = ['/usr/bin/codesign', '--force', '--sign', signer]
    if distribution:
        command += ['--timestamp']
        if executable:
            command += ['--options', 'runtime']
            if not entitlements:
                command += ['--preserve-metadata=entitlements']
    else:
        command += ['--preserve-metadata=flags' if entitlements else '--preserve-metadata=entitlements,flags']
    if entitlements:
        command += ['--entitlements', entitlements]
    if keychain:
        command += ['--keychain', keychain]
        if not distribution:
            command += ['--timestamp=none']
    return command


def verify_distribution(path, executable=True):
    display = subprocess.run(['/usr/bin/codesign', '--display', '--verbose=4', str(path)],
                             capture_output=True, text=True)
    if display.returncode:
        raise RuntimeError(f'Cannot inspect the distribution signature: {path}')
    detail = display.stdout + display.stderr
    if not re.search(r'^Authority=Developer ID Application: .+ \([A-Z0-9]{10}\)$', detail, re.MULTILINE):
        raise RuntimeError(f'Distribution artifact lacks a Developer ID Application signature: {path}')
    if not re.search(r'^Timestamp=.+$', detail, re.MULTILINE):
        raise RuntimeError(f'Distribution artifact lacks a secure timestamp: {path}')
    if executable and not re.search(r'^CodeDirectory .*flags=.*\bruntime\b', detail, re.MULTILINE):
        raise RuntimeError(f'Distribution executable lacks hardened runtime: {path}')


def verify_audio_entitlement(path):
    # The audio helper accesses microphone APIs directly. The containing app is
    # responsible for its capture flow and TCC usage description. Sign both
    # executables explicitly; entitlements are not inherited across exec().
    output = run(['/usr/bin/codesign', '--display', '--entitlements', '-', '--xml', path])
    try:
        entitlements = plistlib.loads(output.encode())
    except plistlib.InvalidFileException:
        raise RuntimeError(f'Missing signed audio-input entitlement: {path}') from None
    if not isinstance(entitlements, dict) or entitlements.get(AUDIO_INPUT) is not True:
        raise RuntimeError(f'Missing signed audio-input entitlement: {path}')


def sign(paths, signer, keychain, distribution=False, verify_only=False):
    for path, identifier in paths:
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f'Missing or invalid packaged executable: {path}')
        if not verify_only:
            entitlements = audio_entitlements_file() if identifier == 'dev.thesaurus.audio' else None
            command = signing_command(signer, keychain, distribution, entitlements=entitlements) + ['--identifier', identifier]
            run(command + [path])
        run(['/usr/bin/codesign', '--verify', '--strict', path])
        if distribution:
            verify_distribution(path)
        if identifier == 'dev.thesaurus.audio':
            verify_audio_entitlement(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--create-local-identity', action='store_true')
    parser.add_argument('--resources', type=Path, help='Sign generated service/audio/transcription resources before bundling')
    parser.add_argument('--app', type=Path, help='Sign a stopped/staged macOS app bundle inside out')
    parser.add_argument('--dmg', type=Path, help='Sign a completed disk image after the app has been signed and stapled')
    parser.add_argument('--allow-ad-hoc', action='store_true')
    parser.add_argument('--distribution', action='store_true', help='Require Developer ID, hardened runtime and secure timestamps; never use a local fallback')
    parser.add_argument('--check-identity', action='store_true', help='Check the distribution identity without signing anything')
    parser.add_argument('--verify-only', action='store_true', help='Verify distribution signatures without accessing a signing identity')
    args = parser.parse_args()
    if args.distribution and (args.create_local_identity or args.allow_ad_hoc):
        parser.error('--distribution cannot create a local identity or allow ad hoc signing')
    if (args.check_identity or args.verify_only) and not args.distribution:
        parser.error('--check-identity and --verify-only require --distribution')
    artifacts = sum(bool(value) for value in (args.resources, args.app, args.dmg))
    if args.check_identity:
        if artifacts or args.verify_only:
            parser.error('--check-identity cannot be combined with artifact paths or --verify-only')
        signer, _, _ = distribution_identity()
        print(f'Developer ID Application identity available: {signer}')
        return
    if artifacts > 1:
        parser.error('Choose only one of --resources, --app or --dmg')
    if args.create_local_identity:
        create_local()
    if not artifacts:
        if args.create_local_identity: return
        parser.error('Choose --resources, --app or --dmg')
    signer, keychain, password = (None, None, None) if args.verify_only else identity(args.allow_ad_hoc, args.distribution)
    if password:
        run(['/usr/bin/security', 'unlock-keychain', '-p', password, keychain], True)
    if args.dmg:
        if args.dmg.is_symlink() or not args.dmg.is_file() or args.dmg.suffix.lower() != '.dmg':
            raise RuntimeError('Disk image must be an existing regular .dmg file')
        dmg = args.dmg.resolve()
        if not args.verify_only:
            run(signing_command(signer, keychain, args.distribution, executable=False) + [dmg])
        run(['/usr/bin/codesign', '--verify', '--strict', dmg])
        if args.distribution:
            verify_distribution(dmg, executable=False)
        print(('Verified ' if args.verify_only else 'Signed and verified ') + str(dmg))
        return
    if args.app and (args.app.is_symlink() or not args.app.is_dir() or args.app.suffix != '.app'):
        raise RuntimeError('App must be an existing .app directory, not a symlink')
    if args.resources and (args.resources.is_symlink() or not args.resources.is_dir()):
        raise RuntimeError('Resources must be an existing directory, not a symlink')
    app = args.app.resolve() if args.app else None
    resources = app/'Contents/Resources/resources' if app else args.resources.resolve()
    targets = [(resources/'thesaurus', 'dev.thesaurus.service'),
               (resources/'thesaurus-audio', 'dev.thesaurus.audio'),
               (resources/'thesaurus-ocr', 'dev.thesaurus.ocr'),
               (resources/'transcription/whisper-cli', 'dev.thesaurus.transcription')]
    sign(targets, signer, keychain, args.distribution, args.verify_only)
    if app:
        info = run(['/usr/libexec/PlistBuddy', '-c', 'Print :CFBundleIdentifier', app/'Contents/Info.plist'])
        if not args.verify_only:
            command = signing_command(signer, keychain, args.distribution,
                                      entitlements=audio_entitlements_file()) + ['--identifier', info]
            run(command+[app])
        run(['/usr/bin/codesign', '--verify', '--deep', '--strict', app])
        verify_audio_entitlement(app)
        if args.distribution:
            verify_distribution(app)
        # Record only public code identity and file hashes, never key material.
        report = {'app':str(app), 'signer':signer, 'persistent':signer!='-', 'signature_valid':True,
                  'designated_requirement':None}
        display = subprocess.run(['/usr/bin/codesign','-d','-r-',str(app)], capture_output=True, text=True, check=True)
        report['designated_requirement'] = (display.stdout + display.stderr).strip()
        if not args.verify_only:
            report['distribution'] = args.distribution
            STATE.mkdir(parents=True, exist_ok=True)
            (STATE/'last-bundle.json').write_text(json.dumps(report,indent=2)+'\n')
    print(('Verified ' if args.verify_only else 'Signed and verified ') + (str(app) if app else str(resources)))


if __name__ == '__main__':
    os.umask(0o077)
    try: main()
    except (RuntimeError, OSError, ValueError, KeyError) as e:
        print(f'Signing failed: {e}', file=sys.stderr)
        sys.exit(1)
