"""Package a release binary without launching it; no user models are included."""
import argparse
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, help='Release binary, relative to the project root')
    parser.add_argument('--arch', choices=['arm64', 'x64', 'universal'])
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    system = platform.system()
    arch = args.arch or ('arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x64')
    if arch == 'universal' and system != 'Darwin':
        raise SystemExit('Universal app packaging requires macOS.')
    label = {'Windows': 'windows', 'Darwin': 'macos', 'Linux': 'linux'}[system]
    binary = root / (args.binary or Path('target/release') / ('glb-viewer.exe' if system == 'Windows' else 'glb-viewer'))
    if not binary.is_file():
        raise SystemExit('Run cargo build --release --locked first.')
    destination = root / 'dist' / f'glb-studio-{label}-{arch}'
    destination.mkdir(parents=True, exist_ok=True)
    if system == 'Darwin':
        app = destination / 'GLB Studio.app'
        contents = app / 'Contents'
        (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
        target = contents / 'MacOS' / 'glb-viewer'
        with (contents / 'Info.plist').open('wb') as f:
            plistlib.dump(dict(
                CFBundleExecutable='glb-viewer',
                CFBundleIdentifier='com.rmashukov.glbstudio',
                CFBundleName='GLB Studio',
                CFBundlePackageType='APPL',
                CFBundleShortVersionString='0.1.0',
                CFBundleVersion='1',
                CFBundleSupportedPlatforms=['MacOSX'],
                LSMinimumSystemVersion=os.environ.get('MACOSX_DEPLOYMENT_TARGET', '11.0'),
                NSHighResolutionCapable=True,
            ), f)
    else:
        target = destination / ('GLB Studio.exe' if system == 'Windows' else 'glb-studio')
    shutil.copy2(binary, target)
    for source, name in [('README.md', 'README.md'), ('assets/OFL-NotoSans.txt', 'OFL-NotoSans.txt'), ('LICENSE', 'LICENSE')]:
        shutil.copy2(root / source, destination / name)
    if system == 'Darwin':
        target.chmod(target.stat().st_mode | 0o111)
        # Ad-hoc signing needs no certificate or account. No app is executed.
        subprocess.run(['codesign', '--force', '--sign', '-', str(app)], check=True)
        subprocess.run(['codesign', '--verify', '--strict', str(app)], check=True)
        archive = str(destination) + '.zip'
        subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(destination), archive], check=True)
    else:
        archive = shutil.make_archive(str(destination), 'zip' if system == 'Windows' else 'gztar', root_dir=destination.parent, base_dir=destination.name)
    print(f'Executable: {target}\nArchive: {archive}')


if __name__ == '__main__':
    main()
