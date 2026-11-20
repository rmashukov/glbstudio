"""Build a Universal macOS app for Intel and Apple Silicon. Never launch the app."""
import os
from pathlib import Path
import platform
import re
import subprocess
import sys


def main():
    if platform.system() != 'Darwin':
        raise SystemExit('Run this build script on macOS with Xcode and stable Rust installed.')
    root = Path(__file__).resolve().parent.parent
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET='11.0')
    env['SDKROOT'] = subprocess.check_output(['xcrun', '--sdk', 'macosx', '--show-sdk-path'], text=True).strip()

    def run(*args):
        print('+ ' + ' '.join(map(str, args)), flush=True)
        subprocess.run(list(map(str, args)), cwd=root, env=env, check=True)

    # Compile the exact embedded Metal sources without opening a graphics window.
    shader_dir = root / 'target' / 'metal-build'
    shader_dir.mkdir(parents=True, exist_ok=True)
    header = '#include <metal_stdlib>\nusing namespace metal;\ntypedef float3 vec3;\n'
    surface = header + (root / 'src/shaders/lighting.inc').read_text() + (root / 'src/shaders/surface.metal').read_text()
    renderer = (root / 'src/render.rs').read_text()
    wire_match = re.search(r'const WIRE_METAL: &str = r#"(.*?)"#;', renderer, re.DOTALL)
    if not wire_match:
        raise SystemExit('Cannot find the embedded wireframe Metal shader.')
    for name, source in [('surface', surface), ('wire', wire_match.group(1))]:
        shader = shader_dir / (name + '.metal')
        shader.write_text(source, encoding='utf-8')
        run('xcrun', '--sdk', 'macosx', 'metal', '-std=metal2.3', '-c', shader, '-o', shader.with_suffix('.air'))

    targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin']
    run('rustup', 'target', 'add', *targets)
    binaries = []
    for target in targets:
        run('cargo', 'build', '--release', '--locked', '--bin', 'glb-viewer', '--target', target)
        binaries.append(root / 'target' / target / 'release' / 'glb-viewer')
    universal = root / 'target' / 'universal-apple-darwin' / 'release' / 'glb-viewer'
    universal.parent.mkdir(parents=True, exist_ok=True)
    run('xcrun', 'lipo', '-create', *binaries, '-output', universal)
    run('xcrun', 'lipo', '-verify_arch', 'arm64', 'x86_64', universal)
    run(sys.executable, 'tools/package.py', '--binary', universal, '--arch', 'universal')


if __name__ == '__main__':
    main()
