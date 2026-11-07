"""Check generated bindings and their consumers, including intentional errors."""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
fixtures = Path(__file__).resolve().parent
generated = root / 'target' / 'conformance'
edge_cases = root / 'target' / 'python-types'
environment = dict(os.environ)
environment['MYPYPATH'] = os.pathsep.join((str(generated), str(edge_cases)))
command = [
    sys.executable, '-m', 'mypy', '--strict', '--python-version', '3.12',
    '--cache-dir', str(root / 'target' / 'mypy-cache'),
    '--no-error-summary', '--no-pretty',
]

subprocess.run(
    [*command, str(generated / 'bindings.py'), str(edge_cases / 'edge_bindings.py'),
     str(root / 'target/regressions/private_names.py'), str(root / 'target/regressions/private_helpers.py'),
     str(root / 'target/regressions/scalar_setters.py'),
     str(fixtures / 'valid.py')],
    cwd=root, env=environment, check=True,
)

negative = fixtures / 'invalid.py'
expected = {
    (number, match.group(1))
    for number, line in enumerate(negative.read_text().splitlines(), 1)
    if (match := re.search(r'# EXPECT: ([\w-]+)', line))
}
result = subprocess.run(
    [*command, str(negative)], cwd=root, env=environment,
    capture_output=True, text=True, check=False,
)
actual = {
    (int(match.group(1)), match.group(2))
    for match in re.finditer(r':(\d+): error: .*\[([\w-]+)\]$', result.stdout, re.MULTILINE)
}
if result.returncode != 1 or actual != expected:
    raise AssertionError(f'expected {expected}, got {actual}\n{result.stdout}\n{result.stderr}')
print('Python typing: strict generated bindings, typed consumers, and negative cases passed')

environment['PYTHONPATH'] = environment['MYPYPATH']
subprocess.run(
    [sys.executable, str(fixtures / 'runtime.py')], cwd=root, env=environment, check=True,
)
