"""Run a bounded native render cycle on supplied models (or the embedded demo)."""
import argparse
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('models', nargs='*')
parser.add_argument('--frames', type=int, default=90)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
exe = root / 'target' / 'release' / ('glb-viewer.exe' if os.name == 'nt' else 'glb-viewer')
env = dict(os.environ, GLB_VIEWER_SMOKE_FRAMES=str(max(9, args.frames)))
failed = False
for model in args.models or [None]:
    start = time.monotonic()
    try:
        result = subprocess.run([str(exe)] + ([model] if model else []), env=env, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=120)
        print((result.stdout + result.stderr).strip(), flush=True)
        print(f'{model or "Demo"}: exit={result.returncode}, {time.monotonic()-start:.1f}s', flush=True)
        failed |= result.returncode != 0
    except subprocess.TimeoutExpired:
        print(f'TIMEOUT: {model}', flush=True)
        failed = True
raise SystemExit(1 if failed else 0)
