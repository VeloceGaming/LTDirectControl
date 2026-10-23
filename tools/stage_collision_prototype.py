"""Stage only the approved experimental build, leaving dist/ and installation untouched."""
import ctypes
import hashlib
import json
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "releases" / "collision-prototype-package"
MOD = OUT / "lt_direct_control"
DLL = ROOT / "releases" / "collision-prototype-target" / "release" / "lt_direct_control.dll"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    if not DLL.is_file():
        raise SystemExit("Build the feature-gated prototype DLL first")
    if b"COLLISION PROTOTYPE " not in DLL.read_bytes() or b"collision_prototype_enabled" not in DLL.read_bytes():
        raise SystemExit("DLL does not contain the prototype switch and report tag")
    allowed = (ROOT / "releases" / "collision-prototype-package").resolve()
    if OUT.resolve() != allowed or not allowed.is_relative_to((ROOT / "releases").resolve()):
        raise SystemExit("Prototype destination escaped releases/")
    if OUT.exists():
        shutil.rmtree(OUT)
    MOD.mkdir(parents=True)
    shutil.copy2(DLL, MOD / DLL.name)
    for folder in ("ui", "font", "sound"):
        shutil.copytree(ROOT / "probe" / folder, MOD / folder)
    info = json.loads((ROOT / "probe" / "mod.mod_info").read_text(encoding="utf-8"))
    info["name"] = "LT Takeover — Collision Prototype (test only)"
    info["description"] = "Experimental collision measurement build; replace the normal copy for testing."
    (MOD / "mod.mod_info").write_text(json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    dll = ctypes.CDLL(str(MOD / DLL.name))
    dll.tfm2_mod_required_abi_level.restype = ctypes.c_uint32
    assert dll.tfm2_mod_required_abi_level() == 6
    dll.tfm2_mod_entry_stable.argtypes = [ctypes.c_void_p]
    dll.tfm2_mod_entry_stable.restype = ctypes.c_void_p
    assert dll.tfm2_mod_entry_stable(None) is None
    archive = OUT / "collision-prototype.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for file in sorted(MOD.rglob("*")):
            if file.is_file():
                z.write(file, file.relative_to(OUT))
    manifest = {
        "purpose": "test-only; never published or installed automatically",
        "mod_id": info["mod_id"], "version": info["version"],
        "feature": "collision-prototype", "dll_sha256": digest(MOD / DLL.name),
        "zip_sha256": digest(archive), "abi": 6, "null_host_rejected": True,
        "files": {f.relative_to(MOD).as_posix(): digest(f)
                  for f in sorted(MOD.rglob("*")) if f.is_file()},
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    with zipfile.ZipFile(archive) as z:
        assert z.testzip() is None
        assert len(z.namelist()) == len(manifest["files"])
    print(json.dumps({"package": str(MOD), "zip": str(archive),
                      "files": len(manifest["files"]), "dll_sha256": manifest["dll_sha256"]}, indent=2))


if __name__ == "__main__":
    main()
