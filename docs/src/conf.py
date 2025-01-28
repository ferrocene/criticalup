# SPDX-FileCopyrightText: The Ferrocene Developers
# SPDX-License-Identifier: MIT OR Apache-2.0


# -- Project information -----------------------------------------------------

project = "CriticalUp Documentation"
copyright = "The Ferrocene Developers"
author = "The Ferrocene Developers"


# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    "ferrocene_toctrees",
    "ferrocene_qualification",
    "ferrocene_domain_cli",
    "myst_parser",
]

# autosectionlabel unique names settings
autosectionlabel_prefix_document = True
ferrocene_substitutions_path = "sphinx-substitutions.toml"
ferrocene_target_names_path = "target-names.toml"
ferrocene_id = "CUD"

# Add any paths that contain templates here, relative to this directory.
templates_path = []

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = []

# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#

html_theme = "ferrocene"
html_theme_path = ["../shared/themes"]

html_theme_options = {
    "license": "MIT or Apache 2.0",
}

html_title = "CriticalUp Documentation"
html_short_title = "CriticalUp Documentation"

# -- Options for linting -----------------------------------------------------

lint_alphabetical_section_titles = ["glossary"]

lint_no_paragraph_ids = ["index"]
