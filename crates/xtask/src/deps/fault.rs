// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Who may turn the fault feature on (ADR 0004, C4, point 3): its home
//! declares it, off by default, and of every other package only `xtask`
//! asks for it, and only in its dependency's feature list. A package's own
//! feature that enables it is forwarding, which every package is refused.
//!
//! These rules read what each manifest in the resolved graph asks for,
//! workspace member or not, and count only declarations that resolve to the
//! home itself. The feature being on without a package's own request, under
//! the all-features lint or by Cargo's feature unification, is not a
//! request.

use super::{FAULT, FAULT_HOME, Violation};
use crate::metadata::{self, Declaration, Package, Workspace};

#[cfg(test)]
mod tests;

/// The only package that may ask for the fault feature: the workspace's
/// automation.
const REQUESTER: &str = "xtask";

/// Each feature of `home` that turns the fault feature on, the feature
/// itself included, with the features between it and `fault` on one path.
fn enabling(home: &Package) -> Vec<(&str, Vec<String>)> {
    home.features
        .iter()
        .filter_map(|(name, _)| path_to_fault(home, name).map(|via| (name.as_str(), via)))
        .collect()
}

/// The features `feature` passes through to reach `fault` in `home`'s own
/// feature map, if it reaches it: empty when it is `fault`.
fn path_to_fault(home: &Package, feature: &str) -> Option<Vec<String>> {
    let mut frontier: Vec<(&str, Vec<String>)> = vec![(feature, Vec::new())];
    let mut seen: Vec<&str> = vec![feature];
    let mut next = 0;
    while let Some((current, via)) = frontier.get(next).cloned() {
        next += 1;
        if current == FAULT {
            return Some(via);
        }
        let enables = home
            .features
            .iter()
            .find(|(name, _)| name == current)
            .map_or(&[][..], |(_, enables)| enables.as_slice());
        for name in enables.iter().map(String::as_str) {
            if seen.contains(&name) {
                continue;
            }
            seen.push(name);
            let mut path = via.clone();
            if current != feature {
                path.push(current.to_owned());
            }
            frontier.push((name, path));
        }
    }
    None
}

/// The declarations of `package` whose own edge resolves to the home,
/// as `metadata::resolved` binds them by Cargo's own names and ids. A
/// declaration of another package named like the home resolves elsewhere,
/// and one Cargo did not resolve is bound to nothing; neither is counted.
fn on_home<'a>(
    workspace: &'a Workspace,
    package: &'a Package,
    home: &Package,
) -> Vec<&'a Declaration> {
    metadata::resolved(workspace, package)
        .into_iter()
        .filter(|(_, to)| to.id == home.id)
        .map(|(declared, _)| declared)
        .collect()
}

/// The fault feature's rules over the resolved graph. With no workspace
/// member named `FAULT_HOME` there is nothing to request.
pub(super) fn violations(workspace: &Workspace) -> Vec<Violation> {
    let Some(home) = workspace
        .packages
        .iter()
        .find(|package| package.member && package.name == FAULT_HOME)
    else {
        return Vec::new();
    };
    let mut violations = Vec::new();
    if !home.features.iter().any(|(name, _)| name == FAULT) {
        violations.push(Violation::FaultUndeclared(home.name.clone()));
    }
    let enabling = enabling(home);
    let turns_on = |feature: &str| enabling.iter().any(|(name, _)| *name == feature);
    if let Some((_, via)) = enabling.iter().find(|(name, _)| *name == "default") {
        violations.push(Violation::FaultByDefault {
            name: home.name.clone(),
            via: via.clone(),
        });
    }
    for package in workspace
        .packages
        .iter()
        .filter(|package| package.id != home.id)
    {
        let on_home = on_home(workspace, package, home);
        let requester = package.member && package.name == REQUESTER;
        let asks = |declared: &Declaration| declared.features.iter().any(|asked| turns_on(asked));
        // A request in a dependency's feature list, optional or not.
        for declared in &on_home {
            for feature in declared.features.iter().filter(|feature| turns_on(feature)) {
                if !requester {
                    violations.push(Violation::FaultRequested {
                        from: package.name.clone(),
                        feature: feature.clone(),
                    });
                }
            }
        }
        // A feature of the package's own that turns it on: forwarding.
        let named = |name: &str, holds: &dyn Fn(&Declaration) -> bool| {
            on_home
                .iter()
                .any(|declared| declared.name == name && holds(declared))
        };
        for (feature, enables) in &package.features {
            for enabled in enables {
                let forwards = if let Some(name) = enabled.strip_prefix("dep:") {
                    named(name, &|declared| declared.optional && asks(declared))
                } else if let Some((name, asked)) = enabled.split_once('/') {
                    match name.strip_suffix('?') {
                        // A weak request turns on no optional dependency.
                        Some(weak) => turns_on(asked) && named(weak, &|_| true),
                        // A strong one also turns the dependency on, with
                        // the features its declaration asks for.
                        None => named(name, &|declared| {
                            turns_on(asked) || declared.optional && asks(declared)
                        }),
                    }
                } else {
                    false
                };
                if forwards {
                    violations.push(Violation::FaultForwarded {
                        from: package.name.clone(),
                        feature: feature.clone(),
                        enables: enabled.clone(),
                    });
                }
            }
        }
    }
    violations
}
