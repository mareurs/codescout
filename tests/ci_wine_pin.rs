//! The windows-gnu lane's wine pin must cover the whole `wine-devel` family,
//! including the i386 sub-package by architecture.
//!
//! Why this exists. The lane pinned only the metapackage, `winehq-devel=V`, under
//! a comment saying the exact `Depends` "cascades and no sub-package needs naming".
//! It does not. `winehq-devel` Depends `wine-devel (= V)`, which Depends
//! `wine-devel-amd64 (= V)` and `wine-devel-i386 (= V)`, and apt fills every level
//! it was not told about with that package's CANDIDATE, i.e. the newest release. So
//! the lane worked only while V was WineHQ's newest, and died at setup (exit 100,
//! "held broken packages") the moment the next release appeared. That was both red
//! streaks, 09-05..08 at 11.16 and from 09-19 at 11.17. Reproduced 2026-09-24 in a
//! clean ubuntu:24.04, red on the metapackage-only step and green with the family
//! pin. `docs/issues/archive/2026-09-24-wine-lane-dies-at-apt-setup-with-the-pin-published.md`.
//!
//! **What this test is, and is not.** It is a SHAPE test over `ci.yml`'s text. It
//! cannot show that apt resolves, since only a runner or that container can. What it
//! guards is the regression that is easy to make: deleting the preferences pin as
//! redundant next to the explicit `winehq-devel=V`, or narrowing its glob to
//! `wine-devel*`. The second was measured to fail the same way, because a bare glob
//! matches only native-arch packages and leaves `wine-devel-i386` at its candidate.

use std::path::PathBuf;

/// The text of the `Install MinGW + wine` step: from its `- name:` line to the
/// next step's `- ` at the same indentation.
fn wine_install_step() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let yaml = std::fs::read_to_string(&path).expect("read ci.yml");
    let start = yaml
        .find("      - name: Install MinGW + wine")
        .expect("the windows-gnu lane's `Install MinGW + wine` step must exist in ci.yml");
    let rest = &yaml[start + 1..];
    let end = rest.find("\n      - ").map_or(rest.len(), |i| i + 1);
    rest[..end].to_string()
}

/// The step's `run:` body: everything after `run: |`, so a mention in the comment
/// block above it cannot satisfy an assertion about what the step EXECUTES.
fn wine_install_run() -> String {
    let step = wine_install_step();
    let at = step
        .find("run: |")
        .expect("the wine install step must have a `run: |` block");
    step[at..].to_string()
}

#[test]
fn the_wine_pin_covers_the_whole_family_by_architecture() {
    let run = wine_install_run();
    assert!(
        run.contains("/etc/apt/preferences.d/"),
        "the wine install step must write an apt preferences pin, not rely on the \
         metapackage pin alone. Pinning only `winehq-devel=V` breaks at setup whenever \
         WineHQ has published anything newer than V. Step run block:\n{run}"
    );
    assert!(
        run.contains("wine-devel*:i386"),
        "the preferences pin must name the i386 sub-package by arch (`wine-devel*:i386`): \
         a bare `wine-devel*` glob matches only native-arch packages and leaves \
         `wine-devel-i386` at its candidate, which fails the same way. Step run block:\n{run}"
    );
}

/// The pin has to be in place before the install that depends on it: a
/// preferences file written after `apt-get install` pins nothing for that install.
#[test]
fn the_wine_pin_is_written_before_winehq_devel_is_installed() {
    let run = wine_install_run();
    let pin = run
        .find("/etc/apt/preferences.d/")
        .expect("pin must exist (see the_wine_pin_covers_the_whole_family_by_architecture)");
    let install = run
        .find("winehq-devel=${WINE_PIN}")
        .expect("the step must still install `winehq-devel=${WINE_PIN}~…`");
    assert!(
        pin < install,
        "the preferences pin must be written before the winehq-devel install"
    );
}
