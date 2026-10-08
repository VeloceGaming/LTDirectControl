"""Regression tests for relocation masking and private snapshot integrity."""
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
import json
import tempfile
import unittest

import patch_migration as migration


class PatternTests(unittest.TestCase):
    def setUp(self):
        self.cs = migration.capstone.Cs(migration.capstone.CS_ARCH_X86, migration.capstone.CS_MODE_64)
        self.cs.detail = True
        self.base = 0x140000000

    def signature(self, data):
        return migration.masked_signature(list(self.cs.disasm(data, self.base+0x1000)), self.base, 0x6000000)

    def test_relative_calls_mask_only_displacement(self):
        code = bytes.fromhex("e878563412488b83b0020000c3")
        value, mask = self.signature(code)
        self.assertEqual(value, code)
        self.assertEqual(mask[0], 255)
        self.assertEqual(mask[1:5], b"\0"*4)
        self.assertEqual(mask[5:], b"\xff"*8)

    def test_rip_reference_masks_address_not_field_access(self):
        code = bytes.fromhex("488b0512345678488b80b0020000c3")
        _, mask = self.signature(code)
        self.assertEqual(mask[3:7], b"\0"*4)
        self.assertEqual(mask[7:], b"\xff"*8)

    def test_image_pointer_is_masked_but_scalar_is_not(self):
        pointer = bytes.fromhex("48b8002000400100000048c7c028000000c3")
        _, mask = self.signature(pointer)
        self.assertEqual(mask[2:10], b"\0"*8)
        self.assertEqual(mask[10:], b"\xff"*8)

    def test_duplicate_matches_are_retained_as_ambiguous(self):
        value = bytes(range(48))
        mask = b"\xff"*48
        self.assertEqual(migration.pattern_hits(b"padding"+value+b"xx"+value, value, mask), [7, 57])

    def test_changed_field_offset_is_not_a_match(self):
        value = bytes(range(48))
        changed = bytearray(value)
        changed[24] ^= 1
        self.assertEqual(migration.pattern_hits(bytes(changed), value, b"\xff"*48), [])

    def test_relocation_bytes_can_differ(self):
        value = bytes(range(48))
        mask = bytearray(b"\xff"*48)
        mask[16:20] = b"\0"*4
        changed = bytearray(value)
        changed[16:20] = b"\xaa"*4
        self.assertEqual(migration.pattern_hits(bytes(changed), value, bytes(mask)), [0])

    def test_weak_short_prologue_does_not_yield_candidate(self):
        value = bytes.fromhex("5541574156415541545657534883ec38")
        self.assertEqual(migration.pattern_hits(value*3, value, b"\xff"*len(value)), [])

    def test_hit_limit_cannot_hide_ambiguity_as_unique(self):
        value = bytes(range(48))
        self.assertEqual(len(migration.pattern_hits(value*50, value, b"\xff"*48)), 33)


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        # Temporary cleanup is restricted to this generated research directory.
        self.temporary = tempfile.TemporaryDirectory(prefix="migration-tests-", dir=migration.ROOT/"research")
        self.directory = Path(self.temporary.name).resolve()
        assert self.directory.is_relative_to((migration.ROOT/"research").resolve())
        self.game = self.directory/"game"
        (self.game/"mod-sdk-stable").mkdir(parents=True)
        (self.game/"TeamfightManager2.exe").write_bytes(b"executable fixture")
        (self.game/"bundle.game_data").write_bytes(b"bundle fixture")
        (self.game/"mod-sdk-stable/base_version.txt").write_text("0.6.3")
        self.output = self.directory/"snapshot"

    def tearDown(self):
        self.temporary.cleanup()

    def capture(self):
        with redirect_stdout(StringIO()):
            return migration.snapshot(self.game, "fixture", self.output, migration.digest(self.game/"TeamfightManager2.exe"))

    def test_snapshot_verifies_exact_copies(self):
        saved = self.capture()
        self.assertTrue(saved["complete"])
        self.assertEqual(len(saved["files"]), 3)
        self.assertEqual(migration.verify_snapshot(self.output), saved)

    def test_corruption_is_rejected(self):
        self.capture()
        (self.output/"bundle.game_data").write_bytes(b"changed bundle")
        with self.assertRaisesRegex(ValueError, "integrity failed"):
            migration.verify_snapshot(self.output)

    def test_wrong_source_fingerprint_copies_nothing(self):
        with self.assertRaisesRegex(ValueError, "not the expected"):
            migration.snapshot(self.game, "wrong", self.output, "0"*64)
        self.assertFalse(self.output.exists())

    def test_existing_snapshot_is_not_overwritten(self):
        self.capture()
        (self.game/"TeamfightManager2.exe").write_bytes(b"another version")
        with self.assertRaisesRegex(ValueError, "immutable snapshot"):
            self.capture()
        self.assertEqual((self.output/"TeamfightManager2.exe").read_bytes(), b"executable fixture")

    def test_manifest_path_escape_is_rejected(self):
        saved = self.capture()
        saved["files"]["../outside"] = {"bytes": 0, "sha256": "0"*64}
        (self.output/"manifest.json").write_text(json.dumps(saved))
        with self.assertRaisesRegex(ValueError, "escapes"):
            migration.verify_snapshot(self.output)

    def test_compare_rejects_unverified_baseline_before_reading_pe(self):
        with self.assertRaisesRegex(ValueError, "verified baseline"):
            migration.compare(self.game/"TeamfightManager2.exe", self.game/"TeamfightManager2.exe", {"executable_sha256": "0"*64})

    def test_output_cannot_escape_research_directory(self):
        with self.assertRaisesRegex(ValueError, "within this project's research"):
            migration.output_path(migration.ROOT/"probe/migration-snapshot")

    def test_report_cannot_overwrite_snapshot_manifest(self):
        saved = self.capture()
        with self.assertRaisesRegex(ValueError, "not overwrite"):
            migration.save_report(self.output/"manifest.json", {"changed": True})
        self.assertEqual(migration.verify_snapshot(self.output), saved)

    def test_report_cannot_overwrite_preserved_json_asset(self):
        (self.game/"mod-sdk-stable/config.json").write_text('{"original": true}')
        self.capture()
        with self.assertRaisesRegex(ValueError, "not overwrite"):
            migration.save_report(self.output/"mod-sdk-stable/config.json", {"changed": True})

    def test_report_cannot_overwrite_executable(self):
        self.capture()
        with self.assertRaisesRegex(ValueError, ".json filename"):
            migration.save_report(self.output/"TeamfightManager2.exe", {"changed": True})


class RealImageSmokeTest(unittest.TestCase):
    def test_identical_real_function_yields_review_only_candidate(self):
        executable = migration.SNAPSHOTS/"0.6.3/TeamfightManager2.exe"
        if not executable.is_file():
            self.skipTest("Local game snapshot not present; deterministic tests still apply")
        image = migration.Image(executable)
        chosen = None
        for bounds in image.functions:
            if not 100 <= bounds[1]-bounds[0] <= 1000:
                continue
            instructions = image.decoded(bounds)
            value, mask = migration.masked_signature(instructions, image.base, image.image_size)
            if len(value) != bounds[1]-bounds[0]:
                continue
            hits = migration.pattern_hits(image.text, value, mask)
            if len(hits) == 1 and image.text_rva+hits[0] == bounds[0]:
                chosen = bounds
                break
        self.assertIsNotNone(chosen, "No unique real function found for algorithm smoke check")
        receipt = {"executable_sha256": migration.digest(executable), "native_branches": {
            "ALGORITHM_SELF_CHECK": {"rva": hex(chosen[0]), "bytes": image.pe.get_data(chosen[0],16).hex()}}}
        with redirect_stdout(StringIO()):
            result = migration.compare(executable, executable, receipt)
        row = result["proposals"]["ALGORITHM_SELF_CHECK"]
        self.assertEqual(row["status"], "unique_candidate_requires_review")
        self.assertEqual(row["candidates"][0]["rva"], hex(chosen[0]))
        self.assertEqual(row["candidates"][0]["basis"], "masked_function")
        self.assertFalse(result["runtime_approved"])
        self.assertFalse(row["runtime_approved"])


if __name__ == "__main__":
    unittest.main()
