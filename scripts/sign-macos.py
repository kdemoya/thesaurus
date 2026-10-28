#!/usr/bin/env python3
"""Sign Thesaurus code inside out, with an optional persistent local identity.

Creating an identity is an explicit operation. Builds never create certificates,
change certificate trust, or reset macOS privacy permissions.
"""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / '.local' / 'signing'
CONFIG = STATE / 'identity.json'


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


def identity(allow_ad_hoc):
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


def sign(paths, signer, keychain):
    for path, identifier in paths:
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f'Missing or invalid packaged executable: {path}')
        command = ['/usr/bin/codesign', '--force', '--sign', signer, '--identifier', identifier,
                   '--preserve-metadata=entitlements,flags']
        if keychain:
            command += ['--keychain', keychain, '--timestamp=none']
        run(command + [path])
        run(['/usr/bin/codesign', '--verify', '--strict', path])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--create-local-identity', action='store_true')
    parser.add_argument('--resources', type=Path, help='Sign generated service/audio/transcription resources before bundling')
    parser.add_argument('--app', type=Path, help='Sign a stopped/staged macOS app bundle inside out')
    parser.add_argument('--allow-ad-hoc', action='store_true')
    args = parser.parse_args()
    if args.create_local_identity:
        create_local()
    if not args.resources and not args.app:
        if args.create_local_identity: return
        parser.error('Choose --resources or --app')
    signer, keychain, password = identity(args.allow_ad_hoc)
    if password:
        run(['/usr/bin/security', 'unlock-keychain', '-p', password, keychain], True)
    app = args.app.resolve() if args.app else None
    resources = app/'Contents/Resources/resources' if app else args.resources.resolve()
    targets = [(resources/'thesaurus', 'dev.thesaurus.service'),
               (resources/'thesaurus-audio', 'dev.thesaurus.audio'),
               (resources/'transcription/whisper-cli', 'dev.thesaurus.transcription')]
    sign(targets, signer, keychain)
    if app:
        info = run(['/usr/libexec/PlistBuddy', '-c', 'Print :CFBundleIdentifier', app/'Contents/Info.plist'])
        command = ['/usr/bin/codesign', '--force', '--sign', signer, '--identifier', info,
                   '--preserve-metadata=entitlements,flags']
        if keychain: command += ['--keychain', keychain, '--timestamp=none']
        run(command+[app])
        run(['/usr/bin/codesign', '--verify', '--deep', '--strict', app])
        # Record only public code identity and file hashes, never key material.
        report = {'app':str(app), 'signer':signer, 'persistent':signer!='-', 'signature_valid':True,
                  'designated_requirement':None}
        display = subprocess.run(['/usr/bin/codesign','-d','-r-',str(app)], capture_output=True, text=True, check=True)
        report['designated_requirement'] = (display.stdout + display.stderr).strip()
        STATE.mkdir(parents=True, exist_ok=True)
        (STATE/'last-bundle.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Signed and verified ' + (str(app) if app else str(resources)))


if __name__ == '__main__':
    os.umask(0o077)
    try: main()
    except (RuntimeError, OSError, ValueError, KeyError) as e:
        print(f'Signing failed: {e}', file=sys.stderr)
        sys.exit(1)
