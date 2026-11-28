// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Who may turn the fault feature on (ADR 0004, C4, point 3): its home
//! declares it, off by default, and of every other crate only `xtask` asks
//! for it, and only in its dependency's feature list. A crate's own feature
//! that enables it is forwarding, which every crate is refused.
//!
//! These rules read what each manifest asks for. The feature being on
//! without a crate's own request, under the all-features lint or by
//! Cargo's feature unification, is not a request.

use super::{FAULT, FAULT_HOME, Violation};
use crate::metadata::{Member, Workspace};

#[cfg(test)]
mod tests;

/// The only crate that may ask for the fault feature.
const REQUESTER: &str = "xtask";

/// Each feature of `home` that turns the fault feature on, the feature
/// itself included, with the features between it and `fault` on one path.
fn enabling(home: &Member) -> Vec<(&str, Vec<String>)> {
    home.features
        .iter()
        .filter_map(|(name, _)| path_to_fault(home, name).map(|via| (name.as_str(), via)))
        .collect()
}

/// The features `feature` passes through to reach `fault` in `home`'s own
/// feature map, if it reaches it: empty when it is `fault`.
fn path_to_fault(home: &Member, feature: &str) -> Option<Vec<String>> {
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

/// The fault feature's rules over the workspace. With no member named
/// `FAULT_HOME` there is nothing to request.
pub(super) fn violations(workspace: &Workspace) -> Vec<Violation> {
    let Some(home) = workspace
        .members
        .iter()
        .find(|member| member.name == FAULT_HOME)
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
    for member in workspace
        .members
        .iter()
        .filter(|member| member.name != FAULT_HOME)
    {
        let on_home: Vec<_> = member
            .declarations
            .iter()
            .filter(|declared| declared.package == FAULT_HOME)
            .collect();
        // A request in a dependency's feature list. On an optional
        // dependency it takes effect only through the feature that turns
        // the dependency on, which is forwarding (below).
        for declared in on_home.iter().filter(|declared| !declared.optional) {
            for feature in declared.features.iter().filter(|feature| turns_on(feature)) {
                if member.name != REQUESTER {
                    violations.push(Violation::FaultRequested {
                        from: member.name.clone(),
                        feature: feature.clone(),
                    });
                }
            }
        }
        for (feature, enables) in &member.features {
            for enabled in enables {
                let forwards = if let Some(name) = enabled.strip_prefix("dep:") {
                    on_home.iter().any(|declared| {
                        declared.optional
                            && declared.name == name
                            && declared.features.iter().any(|asked| turns_on(asked))
                    })
                } else if let Some((name, asked)) = enabled.split_once('/') {
                    let name = name.strip_suffix('?').unwrap_or(name);
                    turns_on(asked) && on_home.iter().any(|declared| declared.name == name)
                } else {
                    false
                };
                if forwards {
                    violations.push(Violation::FaultForwarded {
                        from: member.name.clone(),
                        feature: feature.clone(),
                        enables: enabled.clone(),
                    });
                }
            }
        }
    }
    violations
}
