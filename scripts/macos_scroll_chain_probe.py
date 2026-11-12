#!/usr/bin/env python3
"""A page in a scroll view hands on the scrolls it can't use (macOS,
`WebView::set_scroll_chain`), by scroll gestures sent to the app's own window.

    python3 scripts/macos_scroll_chain_probe.py [--target-dir DIR]

Builds the scroll_chain_probe example and runs it. Over a page that fits,
with no chain, the page keeps a gesture and the view under it stays; chaining
every way, the view scrolls. Over a page taller than its box, chaining as its
edges say, the page scrolls and the view stays until the page is at its
bottom, and then the view scrolls on; a gesture back up then scrolls the
page.

The scrolls go to the probe's own window (`-[NSWindow sendEvent:]`): the
user's trackpad and other apps hear nothing. It is launched as a direct
child, from /tmp, with a clean environment. macOS only.
"""
import argparse
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--target-dir', default=os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
    args = parser.parse_args()
    if platform.system() != 'Darwin':
        print('macOS only')
        return 2
    env = dict(os.environ, CARGO_TARGET_DIR=args.target_dir)
    subprocess.run(['cargo', 'build', '-q', '-p', 'zgx-desktop', '--example', 'scroll_chain_probe', '--locked'],
                   cwd=ROOT, env=env, check=True)
    with tempfile.TemporaryDirectory(prefix='zgx-chain-') as stage:
        binary = pathlib.Path(stage) / 'scroll_chain_probe'
        shutil.copy2(pathlib.Path(args.target_dir) / 'debug/examples/scroll_chain_probe', binary)
        clean = {key: os.environ[key] for key in ('HOME', 'TMPDIR', 'USER') if key in os.environ}
        clean['PATH'] = '/usr/bin:/bin'
        out = subprocess.run([str(binary)], cwd='/tmp', env=clean, capture_output=True, text=True,
                             timeout=90).stdout
    if 'NO_KEY_WINDOW' in out:
        print("the probe's window never became key (another window in front?): run it again")
        return 2
    steps = {}
    for block in re.split(r'^STEP ', out, flags=re.M)[1:]:
        name, _, rest = block.partition('\n')
        view = re.search(r'^VIEW (\S+)$', rest, re.M)
        page = re.search(r'^PAGE .*?(\d+(?:\.\d+)?)', rest, re.M)
        steps[name] = (float(view.group(1)) if view else None, float(page.group(1)) if page else None)
    view = lambda name: steps.get(name, (None, None))[0]
    page = lambda name: steps.get(name, (None, None))[1]
    checks = {
        'a page that fits keeps the gesture without a chain': view('kept') == 0,
        'chaining every way, the view scrolls': (view('fits') or 0) > 100,
        'a taller page scrolls itself first': (page('inside') or 0) > 100 and view('inside') == 0,
        'at its bottom the view scrolls on': (page('edge') or 0) >= 700 and (view('edge') or 0) > 50,
        'the page left mid-gesture scrolls back up': page('back') is not None
        and page('back') < (page('edge') or 0) - 100 and view('back') == 0,
        'done': 'CHAIN_DONE' in out,
    }
    print('\n'.join(line for line in out.splitlines() if re.match(r'(STEP|VIEW|PAGE) ', line)))
    for name, ok in checks.items():
        print(('ok  ' if ok else 'FAIL') + ' ' + name)
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    sys.exit(main())
