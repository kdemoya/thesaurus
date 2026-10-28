#!/usr/bin/env python3
"""Exercise release signing/notarization fail-closed behavior without signing or uploads."""
import importlib.util
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parent
sys.dont_write_bytecode = True


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), SCRIPTS/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


signing = load('sign-macos')
notary = load('notarize-macos')
FINGERPRINT = 'A'*40
DEVELOPER_ID = 'Developer ID Application: Example Developer (A1B2C3D4E5)'
IDENTITY_OUTPUT = f'  1) {FINGERPRINT} "{DEVELOPER_ID}"\n     1 valid identities found'
SIGNED_METADATA = ('CodeDirectory v=20500 size=400 flags=0x10000(runtime) hashes=10+7 location=embedded\n'
                   f'Authority={DEVELOPER_ID}\nTimestamp=Oct 28, 2026 at 12:00:00 PM\n')


class SigningTests(unittest.TestCase):
    def test_distribution_requires_explicit_identity_without_reading_local_config(self):
        for requested in ('', '-'):
            with self.subTest(requested=requested), patch.dict(os.environ, {'APPLE_SIGNING_IDENTITY': requested}, clear=True), patch.object(signing, 'run') as run:
                with self.assertRaisesRegex(RuntimeError, 'Distribution requires'):
                    signing.identity(allow_ad_hoc=True, distribution=True)
                run.assert_not_called()

    def test_distribution_resolves_exact_developer_id_to_fingerprint(self):
        with patch.dict(os.environ, {'APPLE_SIGNING_IDENTITY': DEVELOPER_ID}, clear=True), patch.object(signing, 'run', return_value=IDENTITY_OUTPUT):
            self.assertEqual(signing.distribution_identity(), (FINGERPRINT, None, None))

    def test_distribution_rejects_other_certificate_types_even_by_fingerprint(self):
        for name in ('Apple Development: Example Developer (A1B2C3D4E5)', 'Thesaurus Local Development', 'Developer ID Installer: Example Developer (A1B2C3D4E5)'):
            output = f'  1) {FINGERPRINT} "{name}"'
            with self.subTest(name=name), patch.dict(os.environ, {'APPLE_SIGNING_IDENTITY': FINGERPRINT}, clear=True), patch.object(signing, 'run', return_value=output):
                with self.assertRaisesRegex(RuntimeError, 'exactly one available Developer ID Application'):
                    signing.distribution_identity()

    def test_distribution_uses_explicit_keychain_without_unlocking(self):
        with patch.dict(os.environ, {'APPLE_SIGNING_IDENTITY': FINGERPRINT, 'THESAURUS_SIGNING_KEYCHAIN': '/example/build.keychain-db'}, clear=True), patch.object(signing, 'run', return_value=IDENTITY_OUTPUT) as run:
            self.assertEqual(signing.distribution_identity(), (FINGERPRINT, '/example/build.keychain-db', None))
            run.assert_called_once_with(['/usr/bin/security', 'find-identity', '-v', '-p', 'codesigning', '/example/build.keychain-db'])

    def test_distribution_rejects_missing_and_ambiguous_identity(self):
        for output in ('0 valid identities found', IDENTITY_OUTPUT+'\n'+IDENTITY_OUTPUT):
            with self.subTest(output=output), patch.dict(os.environ, {'APPLE_SIGNING_IDENTITY': FINGERPRINT}, clear=True), patch.object(signing, 'run', return_value=output):
                with self.assertRaises(RuntimeError):
                    signing.distribution_identity()

    def test_distribution_signing_requires_hardened_runtime_and_timestamp_even_with_keychain(self):
        command = signing.signing_command(FINGERPRINT, '/example/build.keychain-db', True)
        self.assertIn('--timestamp', command)
        self.assertNotIn('--timestamp=none', command)
        self.assertEqual(command[command.index('--options')+1], 'runtime')
        self.assertIn('--preserve-metadata=entitlements', command)

    def test_dmg_signing_uses_timestamp_without_executable_options(self):
        command = signing.signing_command(FINGERPRINT, None, True, executable=False)
        self.assertIn('--timestamp', command)
        self.assertNotIn('--options', command)

    def test_local_keychain_signing_preserves_existing_behavior(self):
        command = signing.signing_command(FINGERPRINT, '/example/local.keychain-db', False)
        self.assertIn('--timestamp=none', command)
        self.assertIn('--preserve-metadata=entitlements,flags', command)
        self.assertNotIn('--options', command)

    def test_audio_entitlements_contain_only_the_microphone_resource_permission(self):
        path = signing.audio_entitlements_file()
        self.assertEqual(plistlib.loads(path.read_bytes()), {signing.AUDIO_INPUT: True})

    def test_audio_entitlements_are_explicit_for_local_and_distribution_signatures(self):
        for distribution in (False, True):
            with self.subTest(distribution=distribution):
                command = signing.signing_command(FINGERPRINT, None, distribution,
                                                  entitlements=signing.AUDIO_ENTITLEMENTS)
                self.assertEqual(command[command.index('--entitlements')+1], signing.AUDIO_ENTITLEMENTS)
                self.assertFalse(any(str(value).startswith('--preserve-metadata=entitlements') for value in command))
                if distribution:
                    self.assertIn('--timestamp', command)
                    self.assertEqual(command[command.index('--options')+1], 'runtime')

    def test_only_audio_helper_receives_resource_entitlement(self):
        signed = []
        def run(command):
            if '--sign' in command:
                signed.append(command)
            if '--entitlements' in command and '--display' in command:
                return plistlib.dumps({signing.AUDIO_INPUT: True}).decode()
            return ''
        with tempfile.TemporaryDirectory() as tmp:
            audio, service = Path(tmp)/'thesaurus-audio', Path(tmp)/'thesaurus'
            audio.write_bytes(b'audio fixture')
            service.write_bytes(b'service fixture')
            with patch.object(signing, 'run', side_effect=run), patch.object(signing, 'verify_distribution'):
                signing.sign([(audio, 'dev.thesaurus.audio'), (service, 'dev.thesaurus.service')],
                             FINGERPRINT, None, distribution=True)
        self.assertEqual(len(signed), 2)
        self.assertIn('--entitlements', signed[0])
        self.assertNotIn('--entitlements', signed[1])

    def test_audio_verification_accepts_signed_true_entitlement(self):
        with patch.object(signing, 'run', return_value=plistlib.dumps({signing.AUDIO_INPUT: True}).decode()) as run:
            signing.verify_audio_entitlement(Path('/example/Thesaurus.app'))
            # Current codesign otherwise emits a human-readable [Dict] tree.
            self.assertIn('--xml', run.call_args.args[0])

    def test_audio_verification_rejects_missing_false_and_nonboolean_entitlement(self):
        for entitlements in ({}, {signing.AUDIO_INPUT: False}, {signing.AUDIO_INPUT: 1}, {signing.AUDIO_INPUT: 'true'}):
            with self.subTest(entitlements=entitlements), patch.object(signing, 'run', return_value=plistlib.dumps(entitlements).decode()):
                with self.assertRaisesRegex(RuntimeError, 'Missing signed audio-input entitlement'):
                    signing.verify_audio_entitlement(Path('/example/Thesaurus.app'))

    def test_audio_verification_rejects_unsigned_empty_entitlements(self):
        with patch.object(signing, 'run', return_value=''):
            with self.assertRaisesRegex(RuntimeError, 'Missing signed audio-input entitlement'):
                signing.verify_audio_entitlement(Path('/example/Thesaurus.app'))

    def test_distribution_verification_rejects_missing_runtime_or_timestamp_or_developer_id(self):
        for detail in (SIGNED_METADATA.replace('(runtime)', '(none)'),
                       SIGNED_METADATA.replace('Timestamp=', 'Signed Time='),
                       SIGNED_METADATA.replace('Developer ID Application:', 'Apple Development:')):
            response = subprocess.CompletedProcess([], 0, '', detail)
            with self.subTest(detail=detail), patch.object(signing.subprocess, 'run', return_value=response):
                with self.assertRaises(RuntimeError):
                    signing.verify_distribution(Path('/example/Thesaurus.app'))

    def test_distribution_verification_accepts_runtime_signature(self):
        response = subprocess.CompletedProcess([], 0, '', SIGNED_METADATA)
        with patch.object(signing.subprocess, 'run', return_value=response):
            signing.verify_distribution(Path('/example/Thesaurus.app'))


class NotarizationTests(unittest.TestCase):
    def test_submit_rejects_unaccepted_and_incomplete_responses(self):
        for response in ('{"id":"example-id","status":"Invalid"}', '{"id":"example-id","status":"In Progress"}', '{"status":"Accepted"}'):
            with self.subTest(response=response), patch.object(notary, 'run', return_value=response):
                with self.assertRaises(RuntimeError):
                    notary.submit(Path('/example/upload.zip'), 'existing-profile', None, '30m')

    def test_submit_uses_profile_without_credentials(self):
        with patch.object(notary, 'run', return_value='{"id":"example-id","status":"Accepted"}') as run:
            response = notary.submit(Path('/example/upload.zip'), 'existing-profile', '/example/notary.keychain-db', '20m')
            self.assertEqual(response['status'], 'Accepted')
            command = run.call_args.args[0]
            self.assertEqual(command[command.index('--keychain-profile')+1], 'existing-profile')
            self.assertIn('--wait', command)
            self.assertNotIn('--password', command)
            self.assertNotIn('--apple-id', command)

    def test_verification_failure_prevents_upload(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = Path(tmp)/'Thesaurus.app'
            app.mkdir()
            with patch.object(notary, 'run', side_effect=RuntimeError('invalid signature')), patch.object(notary, 'submit') as submit:
                with self.assertRaises(RuntimeError):
                    notary.notarize(app, 'existing-profile')
                submit.assert_not_called()

    def test_rejected_app_is_never_stapled(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = Path(tmp)/'Thesaurus.app'
            app.mkdir()
            with patch.object(notary, 'run', return_value='') as run, patch.object(notary, 'submit', side_effect=RuntimeError('Invalid')):
                with self.assertRaises(RuntimeError):
                    notary.notarize(app, 'existing-profile')
                self.assertFalse(any('stapler' in command.args[0] for command in run.call_args_list))

    def test_app_is_stapled_and_assessed_only_after_acceptance(self):
        events = []
        with tempfile.TemporaryDirectory() as tmp:
            app = Path(tmp)/'Thesaurus.app'
            app.mkdir()
            def run(command):
                events.append(command)
                return ''
            def submit(*args):
                events.append(['accepted'])
                return {'id': 'example-id', 'status': 'Accepted'}
            with patch.object(notary, 'run', side_effect=run), patch.object(notary, 'submit', side_effect=submit):
                report = notary.notarize(app, 'existing-profile')
            self.assertIn('--verify-only', events[0])
            self.assertEqual(events[2], ['accepted'])
            self.assertEqual(events[3][1:3], ['stapler', 'staple'])
            self.assertEqual(events[4][1:3], ['stapler', 'validate'])
            self.assertIn('--assess', events[5])
            self.assertTrue(report['gatekeeper_assessment_passed'])

    def test_dmg_is_submitted_directly_then_stapled_and_assessed(self):
        with tempfile.TemporaryDirectory() as tmp:
            dmg = Path(tmp)/'Thesaurus.dmg'
            dmg.write_bytes(b'test fixture')
            with patch.object(notary, 'run', return_value='') as run, patch.object(notary, 'submit', return_value={'id': 'example-id', 'status': 'Accepted'}) as submit:
                notary.notarize(dmg, 'existing-profile')
                submit.assert_called_once_with(dmg.resolve(), 'existing-profile', None, '30m')
                self.assertEqual(run.call_args.args[0][0], '/usr/sbin/spctl')
                self.assertIn('context:primary-signature', run.call_args.args[0])

    def test_symlink_is_rejected_without_any_external_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp)/'Thesaurus.app'
            target.mkdir()
            link = Path(tmp)/'Link.app'
            link.symlink_to(target)
            with patch.object(notary, 'run') as run:
                with self.assertRaises(RuntimeError):
                    notary.notarize(link, 'existing-profile')
                run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
