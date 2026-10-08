"""Preserve game builds and propose native relocations for human review.

Only local snapshots/reports are written. This tool never runs a game executable,
patches a DLL, edits runtime addresses, or changes the executable guard.
"""
from __future__ import annotations
from paths import GAME_DIR

import argparse
import bisect
import hashlib
import json
import shutil
import struct
import sys
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GAME = GAME_DIR
DEFAULT_RECEIPT = ROOT / "dist/build-0.41.0.json"
SNAPSHOTS = ROOT / "research/game-builds"
sys.path.insert(0, str(ROOT / ".tools/python"))
import capstone
import pefile


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(4 * 1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def now() -> str:
    # Windows Python need not have IANA tzdata installed.
    from datetime import timezone, timedelta
    return datetime.now(timezone(timedelta(hours=8))).isoformat()


def output_path(path: Path) -> Path:
    resolved = path.resolve()
    if not resolved.is_relative_to((ROOT / "research").resolve()):
        raise ValueError("Snapshot/report output must stay within this project's research directory")
    return resolved


def save_json(path: Path, value: dict) -> None:
    path = output_path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def save_report(path: Path, value: dict) -> None:
    path = output_path(path)
    if path.suffix.lower() != ".json":
        raise ValueError("Migration reports must use a .json filename")
    for parent in path.parents:
        manifest_path = parent / "manifest.json"
        if manifest_path.exists() and parent.is_relative_to((ROOT/"research").resolve()):
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            if path == manifest_path or path.relative_to(parent).as_posix() in manifest.get("files", {}):
                raise ValueError("A report must not overwrite preserved snapshot files")
    save_json(path, value)


def source_files(game: Path) -> dict[str, Path]:
    result = {name: game / name for name in ["TeamfightManager2.exe", "bundle.game_data"]}
    sdk = game / "mod-sdk-stable"
    if not sdk.is_dir():
        raise ValueError("Installed mod-sdk-stable directory is missing")
    for file in sorted(sdk.rglob("*")):
        if file.is_symlink():
            raise ValueError(f"SDK contains a symlink: {file}")
        if file.is_file() and not {"target", ".git", "__pycache__"}.intersection(file.relative_to(sdk).parts):
            result[file.relative_to(game).as_posix()] = file
    for name in ["steam_api64.dll", "steam_api.dll"]:
        if (game / name).is_file():
            result[name] = game / name
    app_manifest = game.parent.parent / "appmanifest_3009300.acf"
    if app_manifest.is_file():
        result["steam/appmanifest_3009300.acf"] = app_manifest
    return result


def verify_snapshot(directory: Path) -> dict:
    directory = output_path(directory)
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != 1 or not manifest.get("complete"):
        raise ValueError("Snapshot manifest is incomplete or unsupported")
    if not manifest.get("files") or "TeamfightManager2.exe" not in manifest["files"]:
        raise ValueError("Snapshot has no executable record")
    for relative, record in manifest["files"].items():
        source_path = directory / relative
        file = source_path.resolve()
        if not file.is_relative_to(directory) or source_path.is_symlink():
            raise ValueError(f"Snapshot path escapes its directory: {relative}")
        if file.stat().st_size != record["bytes"] or digest(file) != record["sha256"]:
            raise ValueError(f"Snapshot integrity failed: {relative}")
    return manifest


def snapshot(game: Path, label: str, directory: Path, expected_sha: str | None) -> dict:
    game = game.resolve()
    directory = output_path(directory)
    if directory == game or directory.is_relative_to(game):
        raise ValueError("Snapshot must not write inside the installed game")
    files = source_files(game)
    first_hash = digest(files["TeamfightManager2.exe"])
    if expected_sha and first_hash != expected_sha.lower():
        raise ValueError("Installed executable is not the expected verified build; nothing copied")
    if (directory / "manifest.json").exists():
        existing = verify_snapshot(directory)
        if existing["files"]["TeamfightManager2.exe"]["sha256"] != first_hash:
            raise ValueError("An immutable snapshot of a different executable already uses this directory")
        print(f"Verified existing snapshot: {directory}", flush=True)
        return existing
    directory.mkdir(parents=True, exist_ok=True)
    total = sum(file.stat().st_size for file in files.values())
    if shutil.disk_usage(directory).free < total + 128 * 1024 * 1024:
        raise ValueError("Not enough free space for a full build snapshot")
    manifest = {"schema": 1, "complete": False, "label": label, "captured_at": now(),
                "source": str(game), "private_local_snapshot": True, "files": {}}
    for relative, source in files.items():
        target = (directory / relative).resolve()
        if not target.is_relative_to(directory):
            raise ValueError(f"Invalid destination: {relative}")
        before = source.stat()
        source_hash = digest(source)
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            if target.is_symlink() or digest(target) != source_hash:
                raise ValueError(f"Partial snapshot differs; preserved without overwriting: {relative}")
        else:
            shutil.copy2(source, target)
        after = source.stat()
        if before.st_size != after.st_size or before.st_mtime_ns != after.st_mtime_ns:
            raise ValueError(f"Steam changed a source file during capture: {relative}")
        if digest(target) != source_hash or digest(source) != source_hash:
            raise ValueError(f"Copy/source hash changed during capture: {relative}")
        manifest["files"][relative] = {"bytes": target.stat().st_size, "sha256": source_hash}
        if relative in ["TeamfightManager2.exe", "bundle.game_data"]:
            print(f"Preserved and hash-verified {relative} ({before.st_size:,} bytes)", flush=True)
    if digest(files["TeamfightManager2.exe"]) != first_hash:
        raise ValueError("Executable changed during capture")
    manifest["complete"] = True
    save_json(directory / "manifest.json", manifest)
    verify_snapshot(directory)
    print(f"Verified {len(files)} files, {total:,} bytes in {directory}", flush=True)
    return manifest


class Image:
    def __init__(self, executable: Path):
        self.pe = pefile.PE(str(executable), fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.image_size = self.pe.OPTIONAL_HEADER.SizeOfImage
        sections = {s.Name.rstrip(b"\0").decode(): s for s in self.pe.sections}
        self.text = sections[".text"].get_data()
        self.text_rva = sections[".text"].VirtualAddress
        pdata = sections[".pdata"].get_data()
        self.functions = sorted({(a, b) for a, b, _ in struct.iter_unpack("<III", pdata[:len(pdata)//12*12]) if a and b})
        self.starts = [a for a, _ in self.functions]
        self.by_start = dict(self.functions)
        self.cs = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        self.cs.detail = True
        self.instructions = {}

    def enclosing(self, rva: int) -> tuple[int, int] | None:
        at = bisect.bisect_right(self.starts, rva) - 1
        if at >= 0 and self.functions[at][0] <= rva < self.functions[at][1]:
            return self.functions[at]
        return None

    def decoded(self, bounds: tuple[int, int]) -> list:
        if bounds not in self.instructions:
            a, b = bounds
            self.instructions[bounds] = list(self.cs.disasm(self.pe.get_data(a, b-a), self.base+a))
        return self.instructions[bounds]

    def identity(self) -> dict:
        return {"timestamp": hex(self.pe.FILE_HEADER.TimeDateStamp), "size_of_image": hex(self.image_size),
                "image_base": hex(self.base), "unwind_functions": len(self.functions)}


def masked_signature(instructions: list, image_base: int, image_size: int) -> tuple[bytes, bytes]:
    """Ignore relocation operands; preserve registers, constants and field offsets."""
    value, mask = bytearray(), bytearray()
    for insn in instructions:
        begin = len(value)
        value.extend(insn.bytes)
        mask.extend(b"\xff" * insn.size)
        branch = insn.group(capstone.CS_GRP_CALL) or insn.group(capstone.CS_GRP_JUMP)
        for operand in insn.operands:
            if operand.type == capstone.x86.X86_OP_MEM and operand.mem.base == capstone.x86.X86_REG_RIP:
                mask[begin+insn.disp_offset:begin+insn.disp_offset+insn.disp_size] = b"\0" * insn.disp_size
            if operand.type == capstone.x86.X86_OP_IMM and (branch or image_base <= operand.imm < image_base+image_size):
                mask[begin+insn.imm_offset:begin+insn.imm_offset+insn.imm_size] = b"\0" * insn.imm_size
    return bytes(value), bytes(mask)


def longest_literal(value: bytes, mask: bytes) -> tuple[int, bytes]:
    best_start, best_end, start = 0, 0, 0
    for i in range(len(mask)+1):
        if i == len(mask) or not mask[i]:
            if i-start > best_end-best_start:
                best_start, best_end = start, i
            start = i+1
    # A fixed window limits temporary needle allocation on giant functions.
    return best_start, value[best_start:min(best_end, best_start+48)]


def pattern_hits(haystack: bytes, value: bytes, mask: bytes, limit: int = 33) -> list[int]:
    if len(value) != len(mask):
        raise ValueError("Pattern/mask sizes differ")
    offset, needle = longest_literal(value, mask)
    if len(needle) < 8 or sum(bool(b) for b in mask) < 32:
        return []  # Too weak to propose a relocation.
    result, at = [], 0
    fixed = [(i, b) for i, b in enumerate(value) if mask[i]]
    while True:
        found = haystack.find(needle, at)
        if found < 0:
            break
        at = found+1
        begin = found-offset
        if begin >= 0 and begin+len(value) <= len(haystack) and all(haystack[begin+i] == b for i, b in fixed):
            result.append(begin)
            if len(result) >= limit:
                break
    return result


def audit(executable: Path, receipt: dict) -> dict:
    image = Image(executable)
    anchors = {}
    for name, a in receipt["native_branches"].items():
        rva, expected = int(a["rva"], 16), bytes.fromhex(a["bytes"])
        actual = image.pe.get_data(rva, len(expected))
        anchors[name] = {"old_rva": hex(rva), "expected": expected.hex(), "actual": actual.hex(), "matches_old_address": actual == expected}
    return {"schema": 1, "generated_at": now(), "executable_sha256": digest(executable),
            "baseline_sha256": receipt["executable_sha256"], "pe": image.identity(),
            "anchors": anchors, "unchanged": sum(a["matches_old_address"] for a in anchors.values()),
            "runtime_approved": False, "warning": "Byte checks only; no native hooks or layouts are approved."}


def compare(old_exe: Path, new_exe: Path, receipt: dict) -> dict:
    if digest(old_exe) != receipt["executable_sha256"]:
        raise ValueError("Old executable is not the verified baseline from the build receipt")
    old, new = Image(old_exe), Image(new_exe)
    proposals, cache = {}, {}
    for name, anchor in receipt["native_branches"].items():
        rva = int(anchor["rva"], 16)
        expected = bytes.fromhex(anchor["bytes"])
        if old.pe.get_data(rva, len(expected)) != expected:
            raise ValueError(f"Baseline anchor differs: {name}")
        bounds = old.enclosing(rva)
        row = {"old_rva": hex(rva), "expected_old_bytes": expected.hex(), "runtime_approved": False}
        if bounds is None:
            proposals[name] = dict(row, status="manual_data_review", candidates=[], reason="Data/vtable record; not a code signature")
            continue
        instructions = old.decoded(bounds)
        positions = [i.address-old.base for i in instructions]
        if rva not in positions:
            proposals[name] = dict(row, status="manual_instruction_review", candidates=[], reason="Anchor does not start at a decoded instruction")
            continue
        if bounds not in cache:
            value, mask = masked_signature(instructions, old.base, old.image_size)
            # Require complete decoding to avoid accepting a partial function.
            hits = pattern_hits(new.text, value, mask) if len(value) == bounds[1]-bounds[0] else []
            cache[bounds] = {
                "starts": [new.text_rva+h for h in hits if new.by_start.get(new.text_rva+h) == new.text_rva+h+len(value)],
                "truncated": len(hits) >= 33,
            }
        candidates = []
        truncated = cache[bounds]["truncated"]
        for start in cache[bounds]["starts"]:
            candidates.append((start+rva-bounds[0], "masked_function"))
        if not candidates:
            index = positions.index(rva)
            context = instructions[max(0, index-16):index+25]
            value, mask = masked_signature(context, old.base, old.image_size)
            offset = rva-(context[0].address-old.base)
            context_hits = pattern_hits(new.text, value, mask)
            truncated = truncated or len(context_hits) >= 33
            for hit in context_hits:
                candidate = new.text_rva+hit+offset
                parent = new.enclosing(candidate)
                context_start = new.text_rva+hit
                if (parent and parent[0] <= context_start and context_start+len(value) <= parent[1]
                        and any(i.address-new.base == candidate for i in new.decoded(parent))):
                    candidates.append((candidate, "masked_context"))
        row["candidates"] = []
        for candidate, basis in candidates:
            detail = {"rva": hex(candidate), "basis": basis, "actual_bytes": new.pe.get_data(candidate, len(expected)).hex()}
            if anchor.get("target") and expected[0] in (0xe8, 0xe9) and len(expected) == 5:
                call = new.pe.get_data(candidate, 5)
                if call[0] in (0xe8, 0xe9):
                    detail["actual_call_target"] = hex(candidate+5+struct.unpack_from("<i", call, 1)[0])
                    detail["old_call_target"] = anchor["target"]
            row["candidates"].append(detail)
        row["search_truncated"] = truncated
        row["status"] = ("candidate_limit_requires_review" if truncated else
                         "unique_candidate_requires_review" if len(candidates) == 1 else
                         "ambiguous" if candidates else "unresolved")
        proposals[name] = row
        print(f"{name}: {row['status']}", flush=True)
    counts = {}
    for row in proposals.values():
        counts[row["status"]] = counts.get(row["status"], 0)+1
    return {"schema": 1, "generated_at": now(), "old_sha256": digest(old_exe), "new_sha256": digest(new_exe),
            "old_pe": old.identity(), "new_pe": new.identity(), "summary": counts, "proposals": proposals,
            "runtime_approved": False, "warning": "Candidates only. Calling conventions, object/vtable layouts, ownership, and semantics still need verification. Nothing was applied."}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    capture = sub.add_parser("snapshot", help="Copy/hash executable, SDK and bundle into private research storage")
    capture.add_argument("--game-dir", type=Path, default=DEFAULT_GAME)
    capture.add_argument("--label", required=True)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--expected-sha256")
    verify = sub.add_parser("verify", help="Recheck every saved snapshot file")
    verify.add_argument("snapshot", type=Path)
    check = sub.add_parser("audit", help="Compare a saved executable with the old address checks")
    check.add_argument("snapshot", type=Path)
    check.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    check.add_argument("--output", type=Path, required=True)
    migration = sub.add_parser("compare", help="Propose relocations, without changing code or guards")
    migration.add_argument("--old", type=Path, required=True)
    migration.add_argument("--new", type=Path, required=True)
    migration.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    migration.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "snapshot":
        snapshot(args.game_dir, args.label, args.output, args.expected_sha256)
    elif args.command == "verify":
        record = verify_snapshot(args.snapshot)
        print(f"Verified snapshot {record['label']}: {len(record['files'])} files", flush=True)
    else:
        receipt = json.loads(args.receipt.read_text(encoding="utf-8"))
        if args.command == "audit":
            verify_snapshot(args.snapshot)
            result = audit(args.snapshot / "TeamfightManager2.exe", receipt)
        else:
            verify_snapshot(args.old)
            verify_snapshot(args.new)
            result = compare(args.old / "TeamfightManager2.exe", args.new / "TeamfightManager2.exe", receipt)
        save_report(args.output, result)
        print(f"Saved review-only report: {args.output}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, FileNotFoundError) as error:
        print(f"Migration stopped: {error}", file=sys.stderr)
        raise SystemExit(2)
