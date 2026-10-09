"""Regression checks for refusing stale or unreviewed migration profiles."""
import copy
import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import verify_native_profile as verifier


class FakePe:
    def get_data(self, rva, size):
        values = {0x10: bytes.fromhex('e80b000000'), 0x30: (0x140000020).to_bytes(8,'little')}
        return values[rva][:size]


class FakeImage:
    pe = FakePe()
    base = 0x140000000

    def __init__(self, path):
        pass

    def identity(self):
        return {'timestamp':'0x123','size_of_image':'0x1000'}


class ProfileTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.exe = self.root/'game.exe'
        self.exe.write_bytes(b'exact reviewed executable fixture')
        self.profile = {
            'schema':1, 'reviewed':True,
            'executable_sha256':hashlib.sha256(self.exe.read_bytes()).hexdigest(),
            'timestamp':'0x123','size_of_image':'0x1000',
            'anchors':{'CALL':{'rva':'0x10','bytes':'e80b000000','target':'0x20'}},
            'layout_anchors':{'TABLE':{'rva':'0x30','pointer_target':'0x20'}},
        }

    def tearDown(self):
        self.temp.cleanup()

    def verify(self, profile=None):
        path = self.root/'profile.json'
        path.write_text(json.dumps(profile or self.profile))
        with patch.object(verifier, 'Image', FakeImage):
            return verifier.verify(path,self.exe,check_sources=False)

    def test_exact_identity_branch_target_and_pointer_pass(self):
        self.assertEqual(self.verify(),self.profile)

    def test_wrong_executable_is_rejected_before_image_parsing(self):
        self.exe.write_bytes(b'updated game')
        path=self.root/'profile.json'
        path.write_text(json.dumps(self.profile))
        with patch.object(verifier,'Image') as decode:
            with self.assertRaisesRegex(ValueError,'fingerprint differs'):
                verifier.verify(path,self.exe,check_sources=False)
            decode.assert_not_called()

    def test_candidate_profile_cannot_be_used_as_reviewed_profile(self):
        p=copy.deepcopy(self.profile);p['reviewed']=False
        with self.assertRaisesRegex(ValueError,'not explicitly reviewed'):
            self.verify(p)

    def test_matching_call_bytes_do_not_authorize_a_different_callee(self):
        p=copy.deepcopy(self.profile);p['anchors']['CALL']['target']='0x21'
        with self.assertRaises(AssertionError):
            self.verify(p)

    def test_changed_layout_bytes_are_rejected(self):
        p=copy.deepcopy(self.profile);p['anchors']['CALL']['bytes']='e80c000000'
        with self.assertRaises(AssertionError):
            self.verify(p)

    def test_changed_optional_tooltip_guard_is_rejected(self):
        p=copy.deepcopy(self.profile)
        p['tooltip_anchors']={'DESCRIPTION_CALL':{'rva':'0x10','bytes':'e80c000000'}}
        with self.assertRaises(AssertionError):
            self.verify(p)

    def test_matching_table_header_does_not_authorize_different_apply_function(self):
        p=copy.deepcopy(self.profile);p['layout_anchors']['TABLE']['pointer_target']='0x21'
        with self.assertRaises(AssertionError):
            self.verify(p)

    def test_reviewed_profile_cannot_verify_stale_rust_addresses(self):
        profile=json.loads((verifier.ROOT/'tools/native_profiles/0.6.3.json').read_text())
        for name in ['native_profile','native_items','native_preview','minimap']:
            target=self.root/f'probe/src/{name}.rs';target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copy2(verifier.ROOT/f'probe/src/{name}.rs',target)
        shutil.copytree(verifier.ROOT/'probe/src/native_adapter',self.root/'probe/src/native_adapter')
        verifier.verify_sources(profile,self.root)
        path=self.root/'probe/src/native_adapter/windows/layout.rs'
        original=path.read_text()
        path.write_text(original.replace('WORKER_SITE: usize = 0xbfc77a','WORKER_SITE: usize = 0xbe470a'))
        with self.assertRaises(AssertionError):
            verifier.verify_sources(profile,self.root)
        path.write_text(original.replace('TOOLTIP_INFO_LOOKUP: usize = 0x19d1bf0',
                                         'TOOLTIP_INFO_LOOKUP: usize = 0x19d1bf1'))
        with self.assertRaises(AssertionError):
            verifier.verify_sources(profile,self.root)


if __name__=='__main__':
    unittest.main()
