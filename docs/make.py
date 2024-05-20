#!/usr/bin/env python3
import os
import subprocess
import sys

root = os.path.abspath(os.path.dirname(__file__))
subprocess.run(
    ["git", "submodule", "update", "--init"],
    check=True,
    cwd=root,
)

sys.path.insert(0, "shared")
import make_common  # noqa: E402

make_common.main(root)
