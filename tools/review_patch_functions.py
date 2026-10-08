"""Read-only paired-function disassembly review; never approves runtime hooks.

Pairs are supplied explicitly after source/call-graph investigation. Alignment
ignores relocated references and field displacements ONLY to expose differences,
not to establish compatibility. Reports retain the original instructions.
"""
import argparse
import difflib
import json
from pathlib import Path

from patch_migration import Image, masked_signature, save_report


def token(insn, image):
    import capstone
    value, mask = masked_signature([insn], image.base, image.image_size)
    mask = bytearray(mask)
    if insn.disp_size and any(
        op.type == capstone.x86.X86_OP_MEM
        and op.mem.base not in (capstone.x86.X86_REG_RSP, capstone.x86.X86_REG_RBP)
        for op in insn.operands
    ):
        mask[insn.disp_offset:insn.disp_offset+insn.disp_size] = bytes(insn.disp_size)
    return bytes(v if m else 0 for v, m in zip(value, mask)).hex()


def review(old, new, pairs):
    report = {"runtime_approved": False, "functions": {}}
    lines = []
    for old_rva, new_rva in pairs.items():
        a, b = int(old_rva, 16), int(new_rva, 16)
        ai, bi = old.decoded(old.enclosing(a)), new.decoded(new.enclosing(b))
        at, bt = [token(i, old) for i in ai], [token(i, new) for i in bi]
        matcher = difflib.SequenceMatcher(a=at, b=bt, autojunk=False)
        mapping, changes = {}, []
        for block in matcher.get_matching_blocks():
            for j in range(block.size):
                x, y = ai[block.a+j], bi[block.b+j]
                mapping[hex(x.address-old.base)] = hex(y.address-new.base)
                if x.op_str != y.op_str:
                    changes.append({"old_rva": hex(x.address-old.base), "new_rva": hex(y.address-new.base),
                                    "old": f"{x.mnemonic} {x.op_str}", "new": f"{y.mnemonic} {y.op_str}"})
        report["functions"][old_rva] = {"new_rva": new_rva, "alignment": matcher.ratio(),
                                       "instruction_mapping": mapping, "changes": changes,
                                       "runtime_approved": False}
        aa = [f"{i.address-old.base:x}: {i.mnemonic} {i.op_str}" for i in ai]
        bb = [f"{i.address-new.base:x}: {i.mnemonic} {i.op_str}" for i in bi]
        lines.extend([f"\nFUNCTION {old_rva} -> {new_rva} alignment={matcher.ratio():.3f}", "OLD"]+aa+["NEW"]+bb)
        print(old_rva, new_rva, f"alignment={matcher.ratio():.3f}", len(ai), len(bi), flush=True)
    return report, "\n".join(lines)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--pairs", type=Path, required=True)
    p.add_argument("--old", type=Path, required=True)
    p.add_argument("--new", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    report, text = review(Image(args.old), Image(args.new), json.loads(args.pairs.read_text()))
    save_report(args.output, report)
    args.output.with_suffix(".txt").write_text(text, encoding="utf-8")
