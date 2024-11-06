#!/usr/bin/env python3
# SPDX-FileCopyrightText: The Ferrocene Developers
# SPDX-License-Identifier: MIT OR Apache-2.0

import os
import subprocess
import sys
import sphinx_shared_resources

root = os.path.abspath(os.path.dirname(__file__))

sphinx_shared_resources.main(root)
