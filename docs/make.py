#!/usr/bin/env -S uv run
# SPDX-FileCopyrightText: The Ferrocene Developers
# SPDX-License-Identifier: MIT OR Apache-2.0

import ferrocene_standalone_make_cli
import os

ferrocene_standalone_make_cli.main(os.path.abspath(os.path.dirname(__file__)))
