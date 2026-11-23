// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the release-assembly reader, on synthetic assembly in the
//! shape the compiler emits for each architecture.

use super::{
    Arch, body, codegen, judge, normalized, profile_departures, profile_table, zero_byte_stores,
};

/// Assembly with one function per harness case, in the shape of
/// `x86_64` output: `wiped` cases with `count` zero stores, a `plain_dead`
/// written `plain`, and `untouched_dead`.
fn x86(count: usize, plain: &str, call_wipe: bool) -> String {
    let stores = "\tmovb\t$0, (%rax)\n".repeat(count);
    let call = if call_wipe {
        "\tcallq\t*_RNvNtCs1_10ironafs_os8mem_wipe4wipe@GOTPCREL(%rip)\n"
    } else {
        ""
    };
    format!(
        "\t.section\t.text._RNvCs9_17ironafs_wipe_gate10wiped_live,\"ax\",@progbits\n\
         _RNvCs9_17ironafs_wipe_gate10wiped_live:\n\t.cfi_startproc\n{call}{stores}\tretq\n\
         .Lfunc_end0:\n\t.size\t_RNvCs9_17ironafs_wipe_gate10wiped_live, .Lfunc_end0-x\n\
         _RNvCs9_17ironafs_wipe_gate10wiped_dead:\n\tcallq\t*__rust_alloc@GOTPCREL(%rip)\n\
         {call}{stores}\tjmpq\t*__rust_dealloc@GOTPCREL(%rip)\n.Lfunc_end1:\n\
         _RNvCs9_17ironafs_wipe_gate10plain_dead:\n\tcallq\t*__rust_alloc@GOTPCREL(%rip)\n\
         \tje\t.LBB2_1\n{plain}.LBB2_1:\n\tjmpq\t*__rust_dealloc@GOTPCREL(%rip)\n.Lfunc_end2:\n\
         _RNvCs9_17ironafs_wipe_gate14untouched_dead:\n\tcallq\t*__rust_alloc@GOTPCREL(%rip)\n\
         \tje\t.LBB3_1\n.LBB3_1:\n\tjmpq\t*__rust_dealloc@GOTPCREL(%rip)\n.Lfunc_end3:\n"
    )
}

#[test]
fn body_holds_the_named_function_s_lines_up_to_its_end() {
    let asm = x86(2, "", false);
    let lines = body(&asm, "wiped_live").unwrap();
    assert_eq!(
        lines,
        [
            "\t.cfi_startproc",
            "\tmovb\t$0, (%rax)",
            "\tmovb\t$0, (%rax)",
            "\tretq"
        ]
    );
    assert!(body(&asm, "nowhere").is_none());
}

#[test]
fn zero_byte_stores_counts_each_architecture_s_form() {
    let x86 = [
        "\tmovb\t$0, (%rax)",
        "\tmovb\t$0, 63(%rax)",
        "\tmovb\t$1, (%rax)",
        "\tmovq\t$0, (%rax)",
    ];
    assert_eq!(zero_byte_stores(&x86, Arch::X86_64), 2);
    let arm = [
        "\tstrb\twzr, [x0]",
        "\tstrb\twzr, [x0, #63]",
        "\tstrb\tw1, [x0]",
        "\tstr\txzr, [x0]",
    ];
    assert_eq!(zero_byte_stores(&arm, Arch::Aarch64), 2);
}

#[test]
fn normalized_drops_directives_labels_and_comments_and_numbering() {
    let lines = [
        "\t.cfi_startproc",
        ".LBB2_1:",
        "\tje\t.LBB2_1",
        "\t#APP",
        "\tjmpq\t*__rust_dealloc@GOTPCREL(%rip)",
    ];
    assert_eq!(
        normalized(&lines),
        ["je .LBB", "jmpq *__rust_dealloc@GOTPCREL(%rip)"]
    );
}

#[test]
fn judge_passes_inlined_volatile_stores_and_a_removed_plain_overwrite() {
    assert_eq!(judge(&x86(64, "", false), Arch::X86_64), Ok(()));
}

#[test]
fn judge_fails_when_wipe_is_called_rather_than_inlined() {
    let problems = judge(&x86(0, "", true), Arch::X86_64).unwrap_err();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("not inlined")),
        "{problems:?}"
    );
}

#[test]
fn judge_fails_when_fewer_zero_stores_than_the_buffer_has_bytes() {
    let problems = judge(&x86(63, "", false), Arch::X86_64).unwrap_err();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("63 zero byte stores")),
        "{problems:?}"
    );
}

#[test]
fn judge_counts_the_case_not_run_when_the_control_keeps_its_stores() {
    let kept = "\tpxor\t%xmm0, %xmm0\n\tmovdqu\t%xmm0, (%rax)\n";
    let problems = judge(&x86(64, kept, false), Arch::X86_64).unwrap_err();
    assert!(
        problems.iter().any(|problem| problem.contains("control")),
        "{problems:?}"
    );
}

#[test]
fn profile_table_takes_the_release_profile_and_nothing_after_it() {
    let manifest = "[workspace]\nmembers = []\n\n[profile.release]\noverflow-checks = true\nlto = \"fat\"\n\n[profile.dev]\nopt-level = 1\n";
    assert_eq!(
        profile_table(manifest),
        Some("[profile.release]\noverflow-checks = true\nlto = \"fat\"\n".to_owned())
    );
    assert_eq!(profile_table("[workspace]\n"), None);
}

#[test]
fn body_follows_a_symbol_the_compiler_merged_into_another() {
    // Two functions that compile to the same code are emitted once, the
    // second as an alias of the first.
    let asm = "_RNvCs9_17ironafs_wipe_gate10plain_dead:\n\tretq\n.Lfunc_end0:\n\
               _RNvCs9_17ironafs_wipe_gate14untouched_dead = _RNvCs9_17ironafs_wipe_gate10plain_dead\n";
    assert_eq!(body(asm, "untouched_dead").unwrap(), ["\tretq"]);
}

/// A line of cargo's verbose output for one rustc call.
fn running(name: &str, options: &str) -> String {
    format!(
        "     Running `/x/rustc --crate-name {name} --edition=2024 src/lib.rs --crate-type lib \
         -C opt-level=3 {options} -C metadata=0a1b -C extra-filename=-0a1b --out-dir /x`\n"
    )
}

#[test]
fn codegen_reads_a_crate_s_options_without_those_that_name_the_build() {
    let log = running("ironafs_os", "-C embed-bitcode=no -Coverflow-checks=on");
    assert_eq!(
        codegen(&log, "ironafs_os"),
        Some(vec![
            "embed-bitcode=no".to_owned(),
            "opt-level=3".to_owned(),
            "overflow-checks=on".to_owned()
        ])
    );
    assert_eq!(codegen(&log, "other"), None);
}

#[test]
fn profile_departures_name_an_option_the_copied_table_dropped() {
    // The shipping build, under a package override the copy did not carry
    // (such as one written `[profile . release . package . ironafs-os]`).
    let shipped = running("ironafs_os", "-C overflow-checks=on -C codegen-units=1");
    let same = format!(
        "{}{}",
        running("ironafs_os", "-C overflow-checks=on -C codegen-units=1"),
        running(
            "ironafs_wipe_gate",
            "-C overflow-checks=on -C codegen-units=1"
        )
    );
    assert_eq!(profile_departures(&shipped, &same), Vec::<String>::new());
    let dropped = format!(
        "{}{}",
        running("ironafs_os", "-C overflow-checks=on"),
        running("ironafs_wipe_gate", "-C overflow-checks=on")
    );
    assert_eq!(profile_departures(&shipped, &dropped).len(), 2);
    assert_eq!(profile_departures("", &same).len(), 1);
}

#[test]
fn every_planted_case_changes_the_clean_harness_or_its_profile() {
    let profile = "[profile.release]\noverflow-checks = true\n";
    let cases = super::self_test::cases(profile);
    assert_eq!(cases.len(), 4);
    for case in cases {
        assert!(
            (case.source == super::HARNESS_SOURCE) != (case.profile == profile),
            "{} plants nothing, or in both the source and the profile",
            case.name
        );
    }
}

#[test]
fn a_plant_counts_only_with_a_failure_saying_it_on_every_target() {
    use super::self_test::as_planted;
    let targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
    let both = [
        "x86_64-unknown-linux-gnu: `wiped_dead` has 0 zero byte stores".to_owned(),
        "aarch64-unknown-linux-gnu: `wiped_dead` has 0 zero byte stores".to_owned(),
    ];
    assert!(as_planted(&both, &targets, "zero byte stores"));
    assert!(!as_planted(&both[..1], &targets, "zero byte stores"));
    assert!(!as_planted(&both, &targets, "refers to ironafs-os"));
}
