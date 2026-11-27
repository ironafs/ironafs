// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Census of written seam implementations in Cargo targets and their modules.
//!
//! Resolves named traits along explicit paths and plain imports. Configuration
//! predicates are carried as source text. Unresolved source is reported
//! alongside the rows, without a claim about compiled target configurations.

pub(super) mod self_test;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, SEAMS, attribute_end, closing, is_punct, item_end, keyword, line};
use crate::Error;
use crate::metadata::{Member, Target, Workspace};
use crate::rust_source::{self, Token, TokenKind, ident, punct};

#[derive(Clone, Copy)]
struct Root<'a> {
    member: &'a Member,
    target: &'a Target,
}

impl Root<'_> {
    fn library(self) -> bool {
        self.member.lib_root.as_ref() == Some(&self.target.root)
    }

    fn label(self) -> String {
        format!("{}:{}", self.target.kinds.join("+"), self.target.name)
    }
}

type ScopeId = usize;

#[derive(Default)]
struct Scope {
    root: usize,
    module: Vec<String>,
    module_parent: Option<ScopeId>,
    block_parent: Option<ScopeId>,
    block: Vec<usize>,
    cfg: Vec<String>,
    bindings: BTreeMap<String, Vec<Binding>>,
    uncertain_imports: BTreeSet<String>,
}

impl Scope {
    fn label(&self) -> String {
        let mut label = "crate".to_owned();
        for module in &self.module {
            label.push_str("::");
            label.push_str(module);
        }
        for block in &self.block {
            let _ = write!(label, "::{{line {block}}}");
        }
        label
    }
}

#[derive(Clone)]
enum Binding {
    Trait(Option<usize>),
    Module(ScopeId),
    Import(Vec<String>),
    Other,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    file: PathBuf,
    line: usize,
}

impl Site {
    fn shown(&self) -> String {
        format!("{}:{}", display_path(&self.file), self.line)
    }
}

struct Implementation {
    scope: ScopeId,
    path: Vec<String>,
    header: String,
    site: Site,
    cfg: Vec<String>,
}

pub(super) struct Source {
    chars: Vec<char>,
    pub(super) tokens: Vec<Token>,
    macro_names: BTreeSet<String>,
}

impl Source {
    pub(super) fn read(file: &Path) -> Result<Result<Self, String>, Error> {
        Ok(rust_source::read_with_source(file)?.map(|(text, tokens)| {
            let macro_names = tokens
                .windows(3)
                .filter_map(|window| {
                    (keyword(window, 0) == Some("macro_rules") && is_punct(window, 1, '!'))
                        .then(|| ident(window.get(2)))
                        .flatten()
                        .map(str::to_owned)
                })
                .collect();
            Self {
                chars: text.chars().collect(),
                tokens,
                macro_names,
            }
        }))
    }

    fn spelling(&self, start: usize, end: usize) -> String {
        let from = self
            .tokens
            .get(start)
            .map_or(self.chars.len(), |token| token.start);
        let to = self
            .tokens
            .get(end)
            .map_or(self.chars.len(), |token| token.start);
        self.chars
            .get(from..to)
            .unwrap_or_default()
            .iter()
            .collect()
    }
}

#[derive(Clone, Copy)]
struct ModuleSource<'a> {
    file: &'a Path,
    child_dir: &'a Path,
    path_dir: &'a Path,
    source: &'a Source,
    ancestors: &'a [PathBuf],
}

#[derive(Default)]
struct Report {
    declarations: BTreeMap<usize, Vec<Site>>,
    implementations: BTreeMap<usize, BTreeSet<String>>,
    unresolved: BTreeSet<(Site, String)>,
}

struct Census<'a> {
    workspace: &'a Workspace,
    canonical_root: &'a Path,
    roots: Vec<Root<'a>>,
    scopes: Vec<Scope>,
    root_scopes: Vec<ScopeId>,
    candidates: Vec<Implementation>,
    report: Report,
    linked: BTreeSet<PathBuf>,
    macros: BTreeMap<(PathBuf, String), (usize, usize)>,
    sources: &'a BTreeMap<PathBuf, Result<Source, String>>,
}

pub(super) fn report(
    workspace: &Workspace,
    canonical_root: &Path,
    sources: &BTreeMap<PathBuf, Result<Source, String>>,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut census = Census {
        workspace,
        canonical_root,
        roots: workspace
            .members
            .iter()
            .flat_map(|member| member.targets.iter().map(|target| Root { member, target }))
            .collect(),
        scopes: Vec::new(),
        root_scopes: Vec::new(),
        candidates: Vec::new(),
        report: Report::default(),
        linked: BTreeSet::new(),
        macros: BTreeMap::new(),
        sources,
    };
    for (root, target) in workspace
        .members
        .iter()
        .flat_map(|member| &member.targets)
        .enumerate()
    {
        let cfg = target
            .required_features
            .iter()
            .map(|feature| format!("required-feature({feature:?})"))
            .collect();
        let scope = census.add_scope(Scope {
            root,
            cfg,
            ..Scope::default()
        });
        census.root_scopes.push(scope);
        census.read_module(scope, &target.root, parent(&target.root), &[])?;
    }
    for (path, source) in sources {
        if !census.linked.contains(path) {
            let shown = census.shown(path);
            if let Err(error) = source {
                return Err(rule(format!(
                    "{}: not readable as Rust: {error}",
                    display_path(&shown)
                )));
            }
            census.note(
                &shown,
                1,
                "unlinked Rust source; no Cargo target module was established".to_owned(),
            );
        }
    }
    census.finish(output)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn parent(file: &Path) -> &Path {
    file.parent().unwrap_or(Path::new(""))
}

impl Census<'_> {
    fn add_scope(&mut self, scope: Scope) -> ScopeId {
        let index = self.scopes.len();
        self.scopes.push(scope);
        index
    }

    fn scope(&self, scope: ScopeId) -> Result<&Scope, Error> {
        self.scopes.get(scope).ok_or(Error::UnexpectedOutput(CHECK))
    }

    fn scope_mut(&mut self, scope: ScopeId) -> Result<&mut Scope, Error> {
        self.scopes
            .get_mut(scope)
            .ok_or(Error::UnexpectedOutput(CHECK))
    }

    fn shown(&self, file: &Path) -> PathBuf {
        file.strip_prefix(self.canonical_root)
            .or_else(|_| file.strip_prefix(&self.workspace.root))
            .unwrap_or(file)
            .to_path_buf()
    }

    fn read_module(
        &mut self,
        scope: ScopeId,
        file: &Path,
        directory: &Path,
        ancestors: &[PathBuf],
    ) -> Result<(), Error> {
        let canonical = file
            .canonicalize()
            .map_err(Error::path_io("locate source", file))?;
        let shown = self.shown(&canonical);
        if !canonical.starts_with(self.canonical_root) {
            self.note(
                &self.shown(file),
                1,
                "module source outside the workspace".to_owned(),
            );
            return Ok(());
        }
        if ancestors.contains(&canonical) {
            return Err(rule(format!(
                "{}: cyclic module reference",
                display_path(&shown)
            )));
        }
        self.linked.insert(canonical.clone());
        let fallback;
        let source = if let Some(source) = self.sources.get(&canonical) {
            source
        } else {
            fallback = Source::read(&canonical)?;
            &fallback
        }
        .as_ref()
        .map_err(|error| {
            rule(format!(
                "{}: not readable as Rust: {error}",
                display_path(&shown)
            ))
        })?;
        validate_delimiters(&shown, &source.tokens)?;
        let mut ancestors = ancestors.to_vec();
        ancestors.push(canonical);
        self.walk(
            scope,
            ModuleSource {
                file: &shown,
                child_dir: directory,
                path_dir: parent(ancestors.last().map_or(file, PathBuf::as_path)),
                source,
                ancestors: &ancestors,
            },
            0..source.tokens.len(),
        )
    }

    fn walk(
        &mut self,
        scope: ScopeId,
        module: ModuleSource<'_>,
        range: std::ops::Range<usize>,
    ) -> Result<(), Error> {
        let ModuleSource { file, source, .. } = module;
        let tokens = &source.tokens;
        let mut at = range.start;
        while is_punct(tokens, at + 1, '!')
            && let Some(end) = attribute_end(tokens, at)
            && end <= range.end
        {
            self.inspect_attribute(file, tokens, at, end);
            if keyword(tokens, at + 3).is_some_and(is_cfg) {
                let cfg = one_line(source.spelling(at + 3, end - 1).trim());
                self.scope_mut(scope)?.cfg.push(cfg);
            }
            at = end;
        }
        while at < range.end {
            if let Some(end) = attribute_end(tokens, at) {
                self.inspect_attribute(file, tokens, at, end);
                at = end;
                continue;
            }
            if let Some(end) = self.opaque_macro_end(scope, module, at, range.start)? {
                at = end;
                continue;
            }
            if let Some(function) = function_keyword(tokens, at)
                && let Some(open) = item_body(tokens, function + 2)
                && let Some(end) = closing(tokens, open)
            {
                let cfg = cfg_at(&self.scope(scope)?.cfg, tokens, at);
                let inner = self.block_scope(scope, line(tokens, function), cfg)?;
                self.walk(inner, module, open + 1..end)?;
                at = end + 1;
                continue;
            }
            match keyword(tokens, at) {
                Some("mod") if ident(tokens.get(at + 1)).is_some() => {
                    at = self.module(scope, module, at)?;
                    continue;
                }
                Some("use") => {
                    let end = item_end(tokens, at);
                    self.imports(scope, file, tokens, at, end)?;
                    at = end;
                    continue;
                }
                Some("trait") => self.trait_at(scope, file, tokens, at)?,
                Some("extern") if keyword(tokens, at + 1) == Some("crate") => {
                    let id = if keyword(tokens, at + 3) == Some("as") {
                        at + 4
                    } else {
                        at + 2
                    };
                    if let Some(name) = ident(tokens.get(id)) {
                        self.scope_mut(scope)?
                            .uncertain_imports
                            .insert(name.to_owned());
                    }
                    at = item_end(tokens, at);
                    continue;
                }
                Some("struct" | "enum" | "union" | "type") => {
                    self.bind(scope, tokens, at, Binding::Other)?;
                }
                Some("impl") => self.implementation(scope, file, source, at)?,
                Some("const" | "static")
                    if ident(tokens.get(at + 1)).is_some()
                        && !matches!(keyword(tokens, at + 1), Some("fn" | "unsafe" | "async"))
                        && !cfg_at(&[], tokens, at).is_empty() =>
                {
                    self.note(
                        file,
                        line(tokens, at),
                        "cfg-gated initializer has unsupported source scope".to_owned(),
                    );
                    at = statement_end(tokens, at + 1, range.end);
                    continue;
                }
                _ => {}
            }
            if let Some(end) = closing(tokens, at) {
                if is_punct(tokens, at, '{') {
                    let cfg = cfg_at(&self.scope(scope)?.cfg, tokens, at);
                    let inner = self.block_scope(scope, line(tokens, at), cfg)?;
                    self.walk(inner, module, at + 1..end)?;
                } else {
                    self.walk(scope, module, at + 1..end)?;
                }
                at = end + 1;
            } else {
                at += 1;
            }
        }
        Ok(())
    }

    fn opaque_macro_end(
        &mut self,
        scope: ScopeId,
        module: ModuleSource<'_>,
        at: usize,
        start: usize,
    ) -> Result<Option<usize>, Error> {
        let ModuleSource { file, source, .. } = module;
        let tokens = &source.tokens;
        if keyword(tokens, at) == Some("macro_rules") && is_punct(tokens, at + 1, '!') {
            if let Some(name) = ident(tokens.get(at + 2)).filter(|name| expression_macro(name)) {
                self.note(
                    file,
                    line(tokens, at),
                    format!("macro definition may shadow {name}!"),
                );
            }
            return Ok(Some(item_end(tokens, at)));
        }
        if let Some(name) = macro_name(tokens, at)
            && let Some(end) = closing(tokens, at + 2)
        {
            let known_shadow = source.macro_names.contains(name);
            let qualified = at
                .checked_sub(1)
                .is_some_and(|previous| is_path_sep(tokens, previous));
            if !expression_macro(name)
                || qualified
                || known_shadow
                || macro_argument_may_supply_items(source, at + 3..end)
                || self.scope(scope)?.block.is_empty() && item_position(tokens, at, start)
            {
                let context = if self.scope(scope)?.block.is_empty() {
                    "item macro"
                } else {
                    "macro in a block"
                };
                let (_, occurrences) = self
                    .macros
                    .entry((file.to_path_buf(), format!("unexpanded {context} {name}!")))
                    .or_insert((line(tokens, at), 0));
                *occurrences += 1;
            }
            return Ok(Some(end + 1));
        }
        Ok(None)
    }

    fn block_scope(
        &mut self,
        scope: ScopeId,
        at: usize,
        cfg: Vec<String>,
    ) -> Result<ScopeId, Error> {
        let outer = self.scope(scope)?;
        let mut block = outer.block.clone();
        block.push(at);
        Ok(self.add_scope(Scope {
            root: outer.root,
            module: outer.module.clone(),
            module_parent: outer.module_parent,
            block_parent: Some(scope),
            block,
            cfg,
            ..Scope::default()
        }))
    }

    fn module(
        &mut self,
        scope: ScopeId,
        context: ModuleSource<'_>,
        at: usize,
    ) -> Result<usize, Error> {
        let ModuleSource {
            file,
            child_dir: directory,
            source,
            ..
        } = context;
        let tokens = &source.tokens;
        let name = ident(tokens.get(at + 1)).unwrap_or_default();
        let outer = self.scope(scope)?;
        let mut module = outer.module.clone();
        module.push(name.to_owned());
        let child_dir = directory.join(name);
        let child = self.add_scope(Scope {
            root: outer.root,
            module,
            module_parent: Some(self.enclosing_module(scope)),
            block: outer.block.clone(),
            cfg: cfg_at(&outer.cfg, tokens, at),
            ..Scope::default()
        });
        self.bind(scope, tokens, at, Binding::Module(child))?;
        if is_punct(tokens, at + 2, '{')
            && let Some(end) = closing(tokens, at + 2)
        {
            if rust_source::attributes_before(tokens, at)
                .iter()
                .any(|attr| {
                    attr.starts_with("path=")
                        || attr.starts_with("cfg_attr(") && attr.contains("path")
                })
            {
                self.note(
                    file,
                    line(tokens, at),
                    format!("redirection of inline module {name}"),
                );
                return Ok(end + 1);
            }
            self.walk(
                child,
                ModuleSource {
                    child_dir: &child_dir,
                    path_dir: &child_dir,
                    ..context
                },
                at + 3..end,
            )?;
            return Ok(end + 1);
        }
        if !is_punct(tokens, at + 2, ';') {
            return Err(rule(format!(
                "{}:{}: malformed module declaration",
                display_path(file),
                line(tokens, at)
            )));
        }

        self.external_module(child, context, at, name, &child_dir)?;
        Ok(at + 3)
    }

    fn external_module(
        &mut self,
        scope: ScopeId,
        context: ModuleSource<'_>,
        at: usize,
        name: &str,
        child_dir: &Path,
    ) -> Result<(), Error> {
        let ModuleSource {
            file,
            child_dir: directory,
            path_dir,
            source,
            ancestors,
        } = context;
        let tokens = &source.tokens;
        let flat = directory.join(format!("{name}.rs"));
        let nested = child_dir.join("mod.rs");
        let attributes = rust_source::attributes_before(tokens, at);
        if attributes
            .iter()
            .any(|attribute| attribute.starts_with("cfg_attr(") && attribute.contains("path"))
        {
            self.note(
                file,
                line(tokens, at),
                format!("conditional redirection of module {name}"),
            );
            return Ok(());
        }
        if let Some(redirect) = attributes
            .iter()
            .find(|attribute| attribute.starts_with("path="))
        {
            if let Some(path) = redirect
                .strip_prefix("path=\"")
                .and_then(|path| path.strip_suffix('"'))
                && !path.contains('\\')
                && !Path::new(path).is_absolute()
            {
                let source_path = path_dir.join(path);
                let canonical = source_path
                    .canonicalize()
                    .map_err(Error::path_io("locate redirected module", &source_path))?;
                let child_dir = parent(&canonical);
                self.read_module(scope, &canonical, child_dir, ancestors)?;
            } else {
                self.note(
                    file,
                    line(tokens, at),
                    format!("unsupported redirection of module {name}"),
                );
            }
        } else {
            match (flat.try_exists(), nested.try_exists()) {
                (Ok(true), Ok(false)) => self.read_module(scope, &flat, child_dir, ancestors)?,
                (Ok(false), Ok(true)) => self.read_module(scope, &nested, child_dir, ancestors)?,
                (Ok(true), Ok(true)) => {
                    return Err(rule(format!(
                        "{}:{}: both {} and {} provide module {name}",
                        display_path(file),
                        line(tokens, at),
                        display_path(&self.shown(&flat)),
                        display_path(&self.shown(&nested))
                    )));
                }
                (Ok(false), Ok(false)) => {
                    return Err(rule(format!(
                        "{}:{}: no source for module {name}; expected {} or {}",
                        display_path(file),
                        line(tokens, at),
                        display_path(&self.shown(&flat)),
                        display_path(&self.shown(&nested))
                    )));
                }
                (Err(error), _) => return Err(Error::path_io("locate module", &flat)(error)),
                (_, Err(error)) => return Err(Error::path_io("locate module", &nested)(error)),
            }
        }
        Ok(())
    }

    fn bind(
        &mut self,
        scope: ScopeId,
        tokens: &[Token],
        at: usize,
        binding: Binding,
    ) -> Result<(), Error> {
        if let Some(name) = ident(tokens.get(at + 1)) {
            self.scope_mut(scope)?
                .bindings
                .entry(name.to_owned())
                .or_default()
                .push(binding);
        }
        Ok(())
    }

    fn imports(
        &mut self,
        scope: ScopeId,
        file: &Path,
        tokens: &[Token],
        at: usize,
        end: usize,
    ) -> Result<(), Error> {
        let tree = tokens.get(at..end).unwrap_or_default();
        for item in rust_source::use_items(tree) {
            if !item.plain {
                for (index, _) in tree.iter().enumerate() {
                    if keyword(tree, index) == Some("as")
                        && let Some(name) = ident(tree.get(index + 1))
                    {
                        self.scope_mut(scope)?
                            .uncertain_imports
                            .insert(name.to_owned());
                    }
                }
                if item.glob {
                    self.note(
                        file,
                        line(tokens, at),
                        "glob import can change trait bindings".to_owned(),
                    );
                }
                if item.renamed {
                    self.note(
                        file,
                        line(tokens, at),
                        "renamed import requires binding evidence".to_owned(),
                    );
                }
                continue;
            }
            let conditional = !cfg_at(&[], tokens, at).is_empty();
            for mut path in item.paths {
                let Some(name) = path.last().cloned() else {
                    continue;
                };
                if expression_macro(&name) {
                    self.note(file, line(tokens, at), format!("import may shadow {name}!"));
                }
                if is_path_sep(tokens, at + 1) {
                    path.insert(0, String::new());
                }
                let binding = if conditional {
                    Binding::Unresolved
                } else {
                    Binding::Import(path)
                };
                self.scope_mut(scope)?
                    .bindings
                    .entry(name)
                    .or_default()
                    .push(binding);
            }
        }
        Ok(())
    }

    fn trait_at(
        &mut self,
        scope: ScopeId,
        file: &Path,
        tokens: &[Token],
        at: usize,
    ) -> Result<(), Error> {
        let current = self.scope(scope)?;
        let root = *self
            .roots
            .get(current.root)
            .ok_or(Error::UnexpectedOutput(CHECK))?;
        let name = ident(tokens.get(at + 1));
        let seam = SEAMS.iter().position(|(seam, home)| {
            name == Some(*seam)
                && root.member.name == *home
                && root.library()
                && current.block.is_empty()
        });
        if let Some(seam) = seam {
            self.report
                .declarations
                .entry(seam)
                .or_default()
                .push(Site {
                    file: file.to_path_buf(),
                    line: line(tokens, at),
                });
        }
        self.bind(scope, tokens, at, Binding::Trait(seam))
    }

    fn implementation(
        &mut self,
        scope: ScopeId,
        file: &Path,
        source: &Source,
        at: usize,
    ) -> Result<(), Error> {
        let tokens = &source.tokens;
        let mut next = at + 1;
        if is_punct(tokens, next, '<') {
            let Some(end) = angles(tokens, next) else {
                return Ok(());
            };
            next = end + 1;
        }
        let (path, next) = trait_path(tokens, next);
        if path.is_empty() || keyword(tokens, next) != Some("for") {
            return Ok(());
        }
        let Some(end) = item_body(tokens, next + 1) else {
            self.note(
                file,
                line(tokens, at),
                "cannot establish end of trait implementation header".to_owned(),
            );
            return Ok(());
        };
        let start = at
            .checked_sub(1)
            .filter(|&previous| matches!(keyword(tokens, previous), Some("unsafe" | "default")))
            .unwrap_or(at);
        let cfg = cfg_at(&self.scope(scope)?.cfg, tokens, start);
        self.candidates.push(Implementation {
            scope,
            path,
            header: one_line(source.spelling(start, end).trim()),
            site: Site {
                file: file.to_path_buf(),
                line: line(tokens, at),
            },
            cfg,
        });
        Ok(())
    }

    fn note(&mut self, file: &Path, line: usize, detail: String) {
        self.report.unresolved.insert((
            Site {
                file: file.to_path_buf(),
                line,
            },
            detail,
        ));
    }

    fn enclosing_module(&self, mut scope: ScopeId) -> ScopeId {
        while let Some(parent) = self.scopes.get(scope).and_then(|scope| scope.block_parent) {
            scope = parent;
        }
        scope
    }

    fn inspect_attribute(&mut self, file: &Path, tokens: &[Token], at: usize, end: usize) {
        let start = at + if is_punct(tokens, at + 1, '!') { 3 } else { 2 };
        let Some(name) = ident(tokens.get(start)) else {
            return;
        };
        let inert = match name {
            "derive" => tokens
                .get(start + 1..end)
                .unwrap_or_default()
                .iter()
                .filter_map(|token| ident(Some(token)))
                .all(|derive| {
                    matches!(
                        derive,
                        "Clone"
                            | "Copy"
                            | "Debug"
                            | "Default"
                            | "Eq"
                            | "Hash"
                            | "Ord"
                            | "PartialEq"
                            | "PartialOrd"
                    )
                }),
            "allow" | "cfg" | "cold" | "default" | "deny" | "deprecated" | "doc" | "expect"
            | "forbid" | "ignore" | "inline" | "link" | "link_name" | "macro_export"
            | "must_use" | "no_std" | "non_exhaustive" | "path" | "repr" | "rustfmt" | "test"
            | "track_caller" | "warn" => true,
            _ => false,
        };
        if !inert {
            self.note(
                file,
                line(tokens, at),
                format!("unresolved attribute {name}"),
            );
        }
    }

    fn binding(&self, scope: ScopeId, name: &str, lexical: bool) -> Option<(ScopeId, &[Binding])> {
        let current = self.scopes.get(scope)?;
        if current.uncertain_imports.contains(name) {
            return Some((scope, &[Binding::Unresolved]));
        }
        if let Some(found) = current.bindings.get(name) {
            return Some((scope, found));
        }
        if lexical && let Some(parent) = current.block_parent {
            return self.binding(parent, name, true);
        }
        None
    }

    fn resolve(&self, scope: ScopeId, path: &[String], lexical: bool, depth: usize) -> Binding {
        let Some((name, rest)) = path.split_first() else {
            return Binding::Other;
        };
        if depth > self.scopes.len() {
            return Binding::Unresolved;
        }
        let Some(current) = self.scopes.get(scope) else {
            return Binding::Unresolved;
        };
        let source = match name.as_str() {
            "crate" => self.root_scopes.get(current.root).copied(),
            "self" => Some(self.enclosing_module(scope)),
            "super" => current.module_parent,
            "" => return self.external(scope, rest, depth),
            _ => None,
        };
        if let Some(source) = source {
            return self.resolve(source, rest, false, depth + 1);
        }
        let result = match self.binding(scope, name, lexical) {
            Some((origin, [Binding::Import(imported)])) => {
                self.resolve(origin, imported, true, depth + 1)
            }
            Some((_, [binding])) => binding.clone(),
            None if lexical => return self.external(scope, path, depth),
            Some(_) => Binding::Unresolved,
            None => Binding::Other,
        };
        match (result, rest.is_empty()) {
            (Binding::Module(child), false) => self.resolve(child, rest, false, depth + 1),
            (Binding::Unresolved, _) => Binding::Unresolved,
            (found, true) => found,
            _ => Binding::Other,
        }
    }

    fn external(&self, scope: ScopeId, path: &[String], depth: usize) -> Binding {
        let Some((name, rest)) = path.split_first() else {
            return Binding::Other;
        };
        let Some(current) = self
            .scopes
            .get(scope)
            .and_then(|scope| self.roots.get(scope.root))
        else {
            return Binding::Unresolved;
        };
        if current.member.renamed.contains(name) {
            return Binding::Unresolved;
        }
        for (root, candidate) in self.roots.iter().copied().enumerate() {
            if !candidate.library() || candidate.target.name != *name {
                continue;
            }
            let dependency = candidate.member.name == current.member.name && !current.library()
                || current.member.declared_dependencies.iter().any(|entry| {
                    entry
                        .split_once(' ')
                        .is_some_and(|(_, name)| name == candidate.member.name)
                });
            if dependency && let Some(scope) = self.root_scopes.get(root) {
                return self.resolve(*scope, rest, false, depth + 1);
            }
        }
        Binding::Other
    }

    fn finish(mut self, output: &mut impl Write) -> Result<(), Error> {
        let macros = std::mem::take(&mut self.macros);
        for ((file, detail), (line, occurrences)) in macros {
            let detail = if occurrences > 1 {
                format!("{detail} ({occurrences} calls in this file)")
            } else {
                detail
            };
            self.note(&file, line, detail);
        }
        for implementation in &self.candidates {
            let scope = self.scope(implementation.scope)?;
            let root = self
                .roots
                .get(scope.root)
                .ok_or(Error::UnexpectedOutput(CHECK))?;
            let resolved = self.resolve(implementation.scope, &implementation.path, true, 0);
            if let Binding::Trait(Some(seam)) = resolved {
                let cfg = if implementation.cfg.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", implementation.cfg.join(" && "))
                };
                let row = format!(
                    "{} | {} | {} | {} | {}{cfg}",
                    root.member.name,
                    root.label(),
                    scope.label(),
                    implementation.header,
                    implementation.site.shown(),
                );
                self.report
                    .implementations
                    .entry(seam)
                    .or_default()
                    .insert(row);
            } else if matches!(resolved, Binding::Unresolved)
                || matches!(resolved, Binding::Other)
                    && implementation
                        .path
                        .last()
                        .is_some_and(|name| SEAMS.iter().any(|(seam, _)| *seam == name))
            {
                self.report.unresolved.insert((
                    implementation.site.clone(),
                    format!("cannot bind trait in {}", implementation.header),
                ));
            }
        }
        for (seam, (name, home)) in SEAMS.iter().enumerate() {
            match self.report.declarations.get(&seam).map(Vec::as_slice) {
                Some([_]) => {}
                Some(sites) => {
                    return Err(rule(format!(
                        "ambiguous declaration of {home}::{name}: {}",
                        sites.iter().map(Site::shown).collect::<Vec<_>>().join(", ")
                    )));
                }
                None => {
                    return Err(rule(format!(
                        "missing declaration of {home}::{name} in its library target"
                    )));
                }
            }
        }
        self.report
            .write(output)
            .map_err(Error::io("report seam census"))
    }
}

impl Report {
    fn write(&self, output: &mut impl Write) -> std::io::Result<()> {
        let status = if self.unresolved.is_empty() {
            "resolved"
        } else {
            "incomplete"
        };
        writeln!(output, "check-seams: written-source census {status}")?;
        for (seam, (name, home)) in SEAMS.iter().enumerate() {
            for site in self.declarations.get(&seam).into_iter().flatten() {
                writeln!(
                    output,
                    "check-seams: declaration {home}::{name} | {}",
                    site.shown()
                )?;
            }
            if let Some(rows) = self.implementations.get(&seam) {
                for row in rows {
                    writeln!(output, "check-seams: implementation {home}::{name} | {row}")?;
                }
            } else {
                writeln!(
                    output,
                    "check-seams: zero identified written implementations of {home}::{name}"
                )?;
            }
        }
        for (site, detail) in &self.unresolved {
            writeln!(
                output,
                "check-seams: unresolved {} | {detail}",
                site.shown()
            )?;
        }
        Ok(())
    }
}

fn cfg_at(inherited: &[String], tokens: &[Token], at: usize) -> Vec<String> {
    let mut cfg = inherited.to_vec();
    cfg.extend(
        rust_source::attributes_before(tokens, at)
            .into_iter()
            .filter(|attribute| {
                attribute.starts_with("cfg(") || attribute.starts_with("cfg_attr(")
            }),
    );
    cfg
}

fn rule(detail: String) -> Error {
    Error::Rules {
        check: CHECK,
        violations: vec![detail],
    }
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ")
}

fn angles(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    let mut at = open;
    while let Some(token) = tokens.get(at) {
        match punct(Some(token)) {
            Some('<') => depth += 1,
            Some('>')
                if !at
                    .checked_sub(1)
                    .is_some_and(|previous| is_punct(tokens, previous, '-')) =>
            {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(at);
                }
            }
            Some('(' | '[' | '{') => at = closing(tokens, at)?,
            _ => {}
        }
        at += 1;
    }
    None
}

fn trait_path(tokens: &[Token], mut at: usize) -> (Vec<String>, usize) {
    let mut result = Vec::new();
    if is_path_sep(tokens, at) {
        result.push(String::new());
        at += 1;
    }
    while let Some(name) = ident(tokens.get(at)) {
        result.push(name.to_owned());
        at += 1;
        if !is_path_sep(tokens, at) {
            break;
        }
        at += 1;
    }
    if is_punct(tokens, at, '<')
        && let Some(end) = angles(tokens, at)
    {
        at = end + 1;
    }
    (result, at)
}

fn is_path_sep(tokens: &[Token], at: usize) -> bool {
    matches!(
        tokens.get(at).map(|token| &token.kind),
        Some(TokenKind::PathSep)
    )
}

fn is_cfg(attribute: &str) -> bool {
    matches!(attribute, "cfg" | "cfg_attr")
}

/// Standard macros that occur in expression positions or produce no
/// trait implementations. This pass does not resolve macro hygiene.
fn expression_macro(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "cfg"
            | "compile_error"
            | "concat"
            | "dbg"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "eprint"
            | "eprintln"
            | "env"
            | "file"
            | "format"
            | "format_args"
            | "include_bytes"
            | "include_str"
            | "line"
            | "matches"
            | "module_path"
            | "option_env"
            | "panic"
            | "print"
            | "println"
            | "stringify"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "vec"
            | "write"
            | "writeln"
    )
}

fn validate_delimiters(file: &Path, tokens: &[Token]) -> Result<(), Error> {
    let mut stack = Vec::new();
    for token in tokens {
        match punct(Some(token)) {
            Some('(') => stack.push((')', token.line)),
            Some('[') => stack.push((']', token.line)),
            Some('{') => stack.push(('}', token.line)),
            Some(close @ (')' | ']' | '}'))
                if stack.pop().is_none_or(|(expected, _)| expected != close) =>
            {
                return Err(rule(format!(
                    "{}:{}: mismatched delimiter {close}",
                    display_path(file),
                    token.line
                )));
            }
            _ => {}
        }
    }
    if let Some((expected, at)) = stack.pop() {
        return Err(rule(format!(
            "{}:{at}: missing closing delimiter {expected}",
            display_path(file)
        )));
    }
    Ok(())
}

fn item_body(tokens: &[Token], mut at: usize) -> Option<usize> {
    while let Some(token) = tokens.get(at) {
        match punct(Some(token)) {
            Some('{') => return Some(at),
            Some(';' | '}' | ')' | ']') => return None,
            Some('<') => at = angles(tokens, at)?,
            Some('(' | '[') => at = closing(tokens, at)?,
            _ => {}
        }
        at += 1;
    }
    None
}

fn function_keyword(tokens: &[Token], mut at: usize) -> Option<usize> {
    for qualifier in ["const", "async", "unsafe"] {
        if keyword(tokens, at) == Some(qualifier) {
            at += 1;
        }
    }
    if keyword(tokens, at) == Some("extern") {
        at += 1;
        if matches!(
            tokens.get(at).map(|token| &token.kind),
            Some(TokenKind::Str(_))
        ) {
            at += 1;
        }
    }
    (keyword(tokens, at) == Some("fn") && ident(tokens.get(at + 1)).is_some()).then_some(at)
}

fn macro_name(tokens: &[Token], at: usize) -> Option<&str> {
    if !is_punct(tokens, at + 1, '!')
        || matches!(
            keyword(tokens, at),
            Some(
                "if" | "while"
                    | "match"
                    | "return"
                    | "else"
                    | "loop"
                    | "for"
                    | "let"
                    | "break"
                    | "continue"
            )
        )
    {
        return None;
    }
    ident(tokens.get(at))
}

fn item_position(tokens: &[Token], at: usize, start: usize) -> bool {
    at == start
        || at
            .checked_sub(1)
            .is_some_and(|previous| matches!(punct(tokens.get(previous)), Some(';' | '}' | ']')))
}

/// Macro arguments are token trees. They can contain item-shaped tokens
/// or another macro that expands to items, so any such evidence belongs
/// to the call's unresolved disposition.
fn macro_argument_may_supply_items(source: &Source, range: std::ops::Range<usize>) -> bool {
    range.into_iter().any(|at| {
        matches!(keyword(&source.tokens, at), Some("impl" | "mod"))
            || macro_name(&source.tokens, at)
                .is_some_and(|name| !expression_macro(name) || source.macro_names.contains(name))
    })
}

fn statement_end(tokens: &[Token], mut at: usize, end: usize) -> usize {
    while at < end {
        if is_punct(tokens, at, ';') {
            return at + 1;
        }
        at = closing(tokens, at).unwrap_or(at) + 1;
    }
    end
}
