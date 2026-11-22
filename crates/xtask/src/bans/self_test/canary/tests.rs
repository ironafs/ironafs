// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the canary self-test's workspace manifest.

use super::without_dependencies;

#[test]
fn without_dependencies_drops_the_table_however_its_header_is_written() {
    for header in [
        "[workspace.dependencies]",
        "[workspace.dependencies] # stated once",
        "[ workspace . dependencies ]",
        "[workspace.\"dependencies\"]",
    ] {
        let manifest = format!(
            "[workspace]\nmembers = [\"crates/*\"]\n\n{header}\na = {{ path = \"a\" }}\n\n[profile.release]\nx = 1\n"
        );
        assert_eq!(
            without_dependencies(&manifest),
            "[workspace]\nmembers = [\"crates/*\"]\n\n[profile.release]\nx = 1\n",
            "{header}"
        );
    }
}
