#!/usr/bin/env python3
"""Package and audit isolated same-source Fob cache variants without replacing releases."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
PROTECTED = {
    "dist/fob-v0.8.2-x86_64-windows.zip": "0cab43bcd4c59abb6d932f95f908b53181342bfcb4eb07c52c975ea082ed317f",
    "dist/fob-perf-v0.1.0-windows.zip": "d60f460856ccfa28f41d62061870c9f95a87abb3d45c9fe1acf0b3be5307b8fd",
}
MONITOR = "src-tauri/src/monitoring/process_monitor.rs"
TYPES = "src-tauri/src/monitoring/types.rs"
INSTRUCTIONS = """Fob 0.8.2 同源对比专用待验证测试包

A 是移除进程元数据缓存优化的对照，B 是保留优化的版本
版本、设置路径 com.hcb.fob、界面和助手载荷不变，这不是新功能版本
A 故意保留旧缓存的 PID 复用和退出清理限制，不作为日常版本使用
对比期间不要使用结束进程等进程操作

先保留当前 Fob 文件夹，不覆盖、不删除、不替换原版本
等确认可以暂停当前 Fob 后，再安排退出和切换，不要同时运行 A 和 B
普通权限运行即可，不要以管理员身份运行，不需要安装、更新或修复助手
两边继续使用同一份现有设置，测试期间不修改设置、助手或驱动
不同文件夹并不代表独立设置，请勿把测试版当成独立配置

只在确认启动的版本哈希和界面状态后附加采样工具
助手开销仍排除，刷新间隔声明为实际值，采样间隔是另一个参数
每次使用新的结果目录，A/B 标签按 comparison.json 中的 variant 填写
旧的 90 秒 B 试采只是工具验证，不与重新启动的 A 直接比较
本包尚未在 Windows 运行验收，构建和静态审计不代表运行通过
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def tree(root):
    return {str(p.relative_to(root)): sha(p.read_bytes()) for p in sorted(root.rglob("*")) if p.is_file()}


def new_file(path, data):
    with path.open("xb") as file:
        file.write(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ("a-root", "b-root", "a-exe", "b-exe", "out", "build-inputs"):
        parser.add_argument("--" + field, type=Path, required=True)
    args = parser.parse_args()
    for name, expected in PROTECTED.items():
        assert sha((ROOT / name).read_bytes()) == expected, name
    assert args.out.resolve() != (ROOT / "dist").resolve(), "use a dedicated output directory"
    args.out.mkdir(parents=True, exist_ok=True)
    base = git(ROOT, "rev-parse", "e9672f1")
    old = git(ROOT, "rev-parse", "ce2adfc^")
    roots = {"A": args.a_root, "B": args.b_root}
    exes = {"A": args.a_exe, "B": args.b_exe}
    for label, root in roots.items():
        assert not git(root, "status", "--porcelain"), f"unclean variant {label}"
        expected_commit = old if label == "A" else base
        for name in (MONITOR, TYPES):
            expected = subprocess.check_output(["git", "-C", str(ROOT), "show", expected_commit + ":" + name])
            actual = (root / name).read_bytes()
            if name == MONITOR:
                expected = expected.split(b"#[cfg(test)]", 1)[0]
                actual = actual.split(b"#[cfg(test)]", 1)[0]
            assert actual == expected, f"unexpected release source in {label}: {name}"
        config = json.loads((root / "src-tauri/tauri.conf.json").read_text())
        assert config["identifier"] == "com.hcb.fob" and config["version"] == "0.8.2"
    runtime_diff = git(args.b_root, "diff", "--name-only", git(args.b_root, "rev-parse", "HEAD"), git(args.a_root, "rev-parse", "HEAD"), "--", "src", "src-tauri", "sensors-helper", "package.json", "package-lock.json")
    assert sorted(runtime_diff.splitlines()) == sorted([MONITOR, TYPES]), runtime_diff
    frontend = tree(args.b_root / "build")
    assert frontend and frontend == tree(args.a_root / "build"), "frontend inputs differ"
    helper_manifest = (args.b_root / "sensors-helper/dist/manifest.sha256").read_bytes()
    assert helper_manifest == (args.a_root / "sensors-helper/dist/manifest.sha256").read_bytes()
    inputs = json.loads(args.build_inputs.read_text())
    spec = importlib.util.spec_from_file_location("check_exe", ROOT / "scripts/check-windows-exe.py")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    with zipfile.ZipFile(ROOT / "dist/fob-v0.8.2-x86_64-windows.zip") as archive:
        assert archive.testzip() is None
        original = {n[len("fob-v0.8.2/"):]: archive.read(n) for n in archive.namelist() if n.startswith("fob-v0.8.2/") and not n.endswith("/")}
    assert sha(original["fob.exe"]) == "b99e8a8d2a66bc99e3b2a07b0ebaf90797531b447b9a1fdf11ce26cde96975b8"
    for line in helper_manifest.decode().splitlines():
        expected, name = line.split("  ", 1)
        assert sha(original[name]) == expected
        for root in roots.values():
            assert (root / "sensors-helper/dist" / name).read_bytes() == original[name]
    report = {"base_source": base, "rollback_source": old, "production_source_difference": runtime_diff.splitlines(), "frontend": frontend, "helper_manifest_sha256": sha(helper_manifest), "protected": PROTECTED, "variants": {}}
    for label, root in roots.items():
        binary = exes[label].read_bytes()
        assert binary != original["fob.exe"] or label == "B"
        assert helper_manifest in binary, "embedded helper pins differ"
        with tempfile.TemporaryDirectory(prefix="fob-variant-audit-", dir=args.out) as temporary:
            stage = Path(temporary)
            for name, data, main_exe in (("fob.exe", binary, True), ("fob-sensors.exe", original["sensors/fob-sensors.exe"], False), ("fob-mft.exe", original["sensors/fob-mft.exe"], False)):
                path = stage / name
                path.write_bytes(data)
                assert checker.check(str(path), "0.8.2", "HCB", main_exe, str(root / "src-tauri/icons/icon.ico") if main_exe else None)
                strings = checker.version_strings(checker.pefile.PE(str(path)))
                assert strings["ProductVersion"] == "0.8.2"
                assert strings["FileVersion"] == ("0.8.2.0" if name == "fob-sensors.exe" else "0.8.2")
            assert sha(original["sensors/fob-mft.exe"]).encode("utf-16le") in original["sensors/fob-sensors.exe"]
        variant = {"variant": label, "cache": "original pid-only cache; copy every frame; no pruning" if label == "A" else "optimized identity cache; shared environment; live pruning", "source_commit": git(root, "rev-parse", "HEAD"), "base_source_commit": base, "rollback_source_commit": old if label == "A" else None, "version": "0.8.2", "identifier": "com.hcb.fob", "identifier_validation": "pinned tauri.conf.json with unchanged generate_context; optimized PE stores identifier as split instruction immediates, not a contiguous string", "exe_sha256": sha(binary), "exe_bytes": len(binary), "original_B_exe_sha256": sha(original["fob.exe"]), "byte_identical_to_original_B": binary == original["fob.exe"], "frontend_tree_sha256": sha(json.dumps(frontend, sort_keys=True).encode()), "helper_manifest_sha256": sha(helper_manifest), "build_inputs": inputs, "windows_runtime_verified": False}
        contents = dict(original)
        contents["fob.exe"] = binary
        contents["comparison.json"] = (json.dumps(variant, indent=2) + "\n").encode()
        contents["对比说明.txt"] = INSTRUCTIONS.encode("utf-8-sig")
        contents["SHA256SUMS.txt"] = "".join(f"{sha(data)}  {name}\n" for name, data in sorted(contents.items()) if name.lower().endswith((".exe", ".dll"))).encode()
        stem = "fob-v0.8.2-" + ("control-A" if label == "A" else "optimized-B")
        output = args.out / (stem + "-windows.zip")
        with output.open("xb") as file, zipfile.ZipFile(file, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, data in sorted(contents.items()):
                archive.writestr(stem + "/" + name, data)
        with zipfile.ZipFile(output) as archive, tempfile.TemporaryDirectory(prefix="fob-roundtrip-", dir=args.out) as temporary:
            assert archive.testzip() is None
            archive.extractall(temporary)
            restored = Path(temporary) / stem
            assert (restored / "fob.exe").read_bytes() == exes[label].read_bytes()
            assert {n[len(stem) + 1:]: archive.read(n) for n in archive.namelist()} == contents
            for line in (restored / "SHA256SUMS.txt").read_text().splitlines():
                expected, name = line.split("  ", 1)
                assert sha((restored / name).read_bytes()) == expected
            for name in original:
                if name not in ("fob.exe", "SHA256SUMS.txt"):
                    assert (restored / name).read_bytes() == original[name], name
        report["variants"][label] = {**variant, "zip": str(output), "zip_sha256": sha(output.read_bytes()), "zip_bytes": output.stat().st_size}
        print(f"PASS {label}: source boundary, compiled executable equality, HCB 0.8.2, asInvoker/icon, helper pins, original non-app payload, CRC/extraction/program checksums")
        print(json.dumps(report["variants"][label], indent=2))
    for name, expected in PROTECTED.items():
        assert sha((ROOT / name).read_bytes()) == expected
    new_file(args.out / "audit.json", (json.dumps(report, indent=2) + "\n").encode())
    print("PASS protected production and sampler archives unchanged")


if __name__ == "__main__":
    main()
