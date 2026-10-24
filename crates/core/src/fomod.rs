//! Reads FOMOD installers (`fomod/ModuleConfig.xml`) and works out, for a set of
//! choices, what they would install, expressed as OpenMW data paths and plugins.
//!
//! FOMOD describes a wizard: steps, each with groups of options; options set
//! flags and install files; later steps can be hidden unless earlier choices
//! set a flag. We read that into plain data ([`FomodInfo`]) so any front end
//! can show it, and [`resolve`] replays the wizard for given [`Choice`]s.
//!
//! Supported: steps with flag-based visibility, all five group kinds, option
//! types (including flag-dependent ones), flags, required and conditional
//! installs, `And`/`Or` conditions. Not supported (reported as problems, never
//! guessed): file/game/version conditions, and installs that rename files or
//! copy into subfolders, because OpenMW data paths cannot express those.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scan::DatapathCandidate;

const MAX_FILE_LEN: u64 = 8 << 20;
const PLUGIN_EXTS: &[&str] = &["esp", "esm", "omwaddon", "omwscripts", "omwgame"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FomodInfo {
    pub module_name: Option<String>,
    /// Folder the installer's relative paths start from (the parent of its
    /// `fomod/` folder), relative to the scanned root. Empty for the root.
    pub base: PathBuf,
    /// The installer's banner picture, relative to the scanned root.
    #[serde(default)]
    pub module_image: Option<PathBuf>,
    /// Installed regardless of choices.
    pub required_files: Vec<FileInstall>,
    pub steps: Vec<Step>,
    /// Installed when their condition holds after all steps.
    pub conditional_installs: Vec<ConditionalInstall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    pub name: String,
    /// Step is shown only when this holds. `None` means always.
    pub visible: Option<Condition>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Group {
    pub name: String,
    pub kind: GroupKind,
    pub options: Vec<FomodOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    SelectAll,
    SelectAny,
    SelectExactlyOne,
    SelectAtLeastOne,
    SelectAtMostOne,
}

impl GroupKind {
    /// Short phrase for showing to a person.
    pub fn describe(self) -> &'static str {
        match self {
            Self::SelectAll => "all are installed",
            Self::SelectAny => "choose any",
            Self::SelectExactlyOne => "choose one",
            Self::SelectAtLeastOne => "choose at least one",
            Self::SelectAtMostOne => "choose at most one",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FomodOption {
    pub name: String,
    pub description: String,
    /// Type when no pattern matches.
    pub default_type: OptionType,
    /// Flag-dependent overrides, first match wins.
    pub type_patterns: Vec<TypePattern>,
    /// Flags set when this option is selected.
    pub sets_flags: Vec<FlagSetting>,
    pub files: Vec<FileInstall>,
    /// The option's picture, relative to the scanned root.
    #[serde(default)]
    pub image: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    Required,
    Recommended,
    Optional,
    CouldBeUsable,
    NotUsable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TypePattern {
    pub condition: Condition,
    pub kind: OptionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FlagSetting {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileInstall {
    /// Relative to [`FomodInfo::base`], with `/` separators.
    pub source: String,
    /// Relative to the data folder, with `/` separators. Empty means the root.
    pub destination: String,
    pub is_folder: bool,
    /// Higher priority overrides lower where files collide.
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConditionalInstall {
    pub condition: Condition,
    pub files: Vec<FileInstall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    And {
        all: Vec<Condition>,
    },
    Or {
        any: Vec<Condition>,
    },
    Flag {
        flag: String,
        value: String,
    },
    /// A condition we do not evaluate (plugin present, game version, ...).
    Unsupported {
        description: String,
    },
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let join = |items: &[Condition], sep: &str| {
            items
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(sep)
        };
        match self {
            Self::And { all } if all.is_empty() => write!(f, "always"),
            Self::And { all } => write!(f, "{}", join(all, " and ")),
            Self::Or { any } => write!(f, "({})", join(any, " or ")),
            Self::Flag { flag, value } => write!(f, "{flag} = {value}"),
            Self::Unsupported { description } => write!(f, "{description} (not evaluated)"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FomodError {
    #[error("not a FOMOD ModuleConfig.xml (root element is not <config>)")]
    NotModuleConfig,
    #[error("invalid XML: {0}")]
    Xml(String),
    #[error("file is larger than {} MB", MAX_FILE_LEN >> 20)]
    TooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Turns file bytes into text. FOMOD files are often UTF-16 with a BOM
/// (Windows tools), sometimes UTF-8 with a BOM, occasionally plain UTF-8.
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| {
                if le {
                    u16::from_le_bytes(c)
                } else {
                    u16::from_be_bytes(c)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Reads and parses a `ModuleConfig.xml`. `base` is recorded in the result.
pub fn read(path: &Path, base: &Path) -> Result<FomodInfo, FomodError> {
    if fs::metadata(path)?.len() > MAX_FILE_LEN {
        return Err(FomodError::TooLarge);
    }
    parse(&decode(&fs::read(path)?), base)
}

pub fn parse(xml: &str, base: &Path) -> Result<FomodInfo, FomodError> {
    let doc = Document::parse(xml).map_err(|e| FomodError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if !is(root, "config") {
        return Err(FomodError::NotModuleConfig);
    }
    let steps = child(root, "installSteps")
        .map(|s| kids(s, "installStep").map(parse_step).collect())
        .unwrap_or_default();
    let conditional_installs = child(root, "conditionalFileInstalls")
        .and_then(|c| child(c, "patterns"))
        .map(|p| {
            kids(p, "pattern")
                .map(|pat| ConditionalInstall {
                    condition: child(pat, "dependencies")
                        .map(parse_condition)
                        .unwrap_or_else(always),
                    files: child(pat, "files").map(parse_files).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut steps: Vec<Step> = steps;
    // Pictures are named relative to the installer's base folder, with backslashes.
    let from_base = |p: PathBuf| base.join(p);
    for option in steps
        .iter_mut()
        .flat_map(|s| s.groups.iter_mut())
        .flat_map(|g| g.options.iter_mut())
    {
        option.image = option.image.take().map(from_base);
    }
    Ok(FomodInfo {
        module_name: child(root, "moduleName").and_then(text),
        module_image: child(root, "moduleImage")
            .and_then(image_path)
            .map(from_base),
        base: base.to_path_buf(),
        required_files: child(root, "requiredInstallFiles")
            .map(parse_files)
            .unwrap_or_default(),
        steps,
        conditional_installs,
    })
}

// ---- XML helpers: tag and attribute names compare case-insensitively ----

fn is(n: Node, name: &str) -> bool {
    n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name)
}

fn kids<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| is(*c, name))
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> Option<Node<'a, 'i>> {
    kids(n, name).next()
}

fn attr(n: Node, name: &str) -> Option<String> {
    n.attributes()
        .find(|a| a.name().eq_ignore_ascii_case(name))
        .map(|a| a.value().to_string())
}

fn text(n: Node) -> Option<String> {
    let t: String = n
        .descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn always() -> Condition {
    Condition::And { all: Vec::new() }
}

// ---- parsing ----

fn parse_step(n: Node) -> Step {
    Step {
        name: attr(n, "name").unwrap_or_default(),
        visible: child(n, "visible")
            .and_then(|v| child(v, "dependencies"))
            .map(parse_condition),
        groups: child(n, "optionalFileGroups")
            .map(|g| kids(g, "group").map(parse_group).collect())
            .unwrap_or_default(),
    }
}

fn parse_group(n: Node) -> Group {
    let kind = match attr(n, "type")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "selectall" => GroupKind::SelectAll,
        "selectany" => GroupKind::SelectAny,
        "selectatleastone" => GroupKind::SelectAtLeastOne,
        "selectatmostone" => GroupKind::SelectAtMostOne,
        _ => GroupKind::SelectExactlyOne,
    };
    Group {
        name: attr(n, "name").unwrap_or_default(),
        kind,
        options: child(n, "plugins")
            .map(|p| kids(p, "plugin").map(parse_option).collect())
            .unwrap_or_default(),
    }
}

fn parse_option(n: Node) -> FomodOption {
    let (default_type, type_patterns) = child(n, "typeDescriptor")
        .map(parse_type)
        .unwrap_or((OptionType::Optional, vec![]));
    FomodOption {
        name: attr(n, "name").unwrap_or_default(),
        description: child(n, "description").and_then(text).unwrap_or_default(),
        default_type,
        type_patterns,
        sets_flags: child(n, "conditionFlags")
            .map(|c| {
                kids(c, "flag")
                    .map(|f| FlagSetting {
                        name: attr(f, "name").unwrap_or_default(),
                        value: text(f).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        files: child(n, "files").map(parse_files).unwrap_or_default(),
        image: child(n, "image").and_then(image_path),
    }
}

/// The `path` of an `<image>` or `<moduleImage>` element, with backslashes made
/// into slashes. `None` if it names nothing.
fn image_path(n: Node) -> Option<PathBuf> {
    let path = attr(n, "path")?.replace('\\', "/");
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn parse_type(n: Node) -> (OptionType, Vec<TypePattern>) {
    if let Some(t) = child(n, "type") {
        return (option_type(&attr(t, "name").unwrap_or_default()), vec![]);
    }
    let Some(dt) = child(n, "dependencyType") else {
        return (OptionType::Optional, vec![]);
    };
    let default = child(dt, "defaultType")
        .map(|t| option_type(&attr(t, "name").unwrap_or_default()))
        .unwrap_or(OptionType::Optional);
    let patterns = child(dt, "patterns")
        .map(|p| {
            kids(p, "pattern")
                .map(|pat| TypePattern {
                    condition: child(pat, "dependencies")
                        .map(parse_condition)
                        .unwrap_or_else(always),
                    kind: child(pat, "type")
                        .map(|t| option_type(&attr(t, "name").unwrap_or_default()))
                        .unwrap_or(OptionType::Optional),
                })
                .collect()
        })
        .unwrap_or_default();
    (default, patterns)
}

fn option_type(s: &str) -> OptionType {
    match s.to_ascii_lowercase().as_str() {
        "required" => OptionType::Required,
        "recommended" => OptionType::Recommended,
        "couldbeusable" => OptionType::CouldBeUsable,
        "notusable" => OptionType::NotUsable,
        _ => OptionType::Optional,
    }
}

fn parse_files(n: Node) -> Vec<FileInstall> {
    n.children()
        .filter(|c| is(*c, "file") || is(*c, "folder"))
        .map(|c| FileInstall {
            source: norm(&attr(c, "source").unwrap_or_default()),
            destination: norm(&attr(c, "destination").unwrap_or_default()),
            is_folder: is(c, "folder"),
            priority: attr(c, "priority")
                .and_then(|p| p.trim().parse().ok())
                .unwrap_or(0),
        })
        .collect()
}

fn parse_condition(n: Node) -> Condition {
    let op_or = attr(n, "operator").is_some_and(|o| o.eq_ignore_ascii_case("or"));
    let items: Vec<Condition> = n
        .children()
        .filter(|c| c.is_element())
        .map(
            |c| match c.tag_name().name().to_ascii_lowercase().as_str() {
                "dependencies" => parse_condition(c),
                "flagdependency" => Condition::Flag {
                    flag: attr(c, "flag").unwrap_or_default(),
                    value: attr(c, "value").unwrap_or_default(),
                },
                other => Condition::Unsupported {
                    description: match other {
                        "filedependency" => format!(
                            "plugin {} is {}",
                            attr(c, "file").unwrap_or_default(),
                            attr(c, "state").unwrap_or_default().to_lowercase()
                        ),
                        "gamedependency" | "fommdependency" => {
                            format!(
                                "{} version {}",
                                other.trim_end_matches("dependency"),
                                attr(c, "version").unwrap_or_default()
                            )
                        }
                        _ => other.to_string(),
                    },
                },
            },
        )
        .collect();
    if op_or {
        Condition::Or { any: items }
    } else {
        Condition::And { all: items }
    }
}

/// `\` to `/`, and no empty, `.` or doubled segments.
fn norm(s: &str) -> String {
    s.replace('\\', "/")
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/")
}

// ---- resolving choices ----

/// One thing the player picked: an option inside a group, optionally pinned to
/// a step when group names repeat. Names compare case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Choice {
    pub step: Option<String>,
    pub group: String,
    pub option: String,
}

impl Choice {
    /// Parses `Group=Option` or `Step/Group=Option`.
    pub fn parse(s: &str) -> Option<Choice> {
        let (key, option) = s.split_once('=')?;
        let (step, group) = match key.split_once('/') {
            Some((st, g)) => (Some(st.trim().to_string()), g.trim().to_string()),
            None => (None, key.trim().to_string()),
        };
        let option = option.trim().to_string();
        (!group.is_empty() && !option.is_empty()).then_some(Choice {
            step,
            group,
            option,
        })
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.step {
            Some(s) => write!(f, "{s}/{}={}", self.group, self.option),
            None => write!(f, "{}={}", self.group, self.option),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FomodProblem {
    /// A choice does not match any option in a visible step, or is not allowed.
    InvalidChoice { choice: String, reason: String },
    /// The player still has to decide, or decided something inconsistent.
    GroupUnresolved {
        step: String,
        group: String,
        cause: Unresolved,
        /// In words, for showing as it is.
        reason: String,
    },
    /// A condition we cannot evaluate was treated as false.
    UnsupportedCondition { description: String },
    /// An install cannot be expressed as an OpenMW data path.
    NotRepresentable { source: String, reason: String },
    /// An install points at something the scan did not find.
    SourceNotFound { source: String },
}

/// Why a group of options is not settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Unresolved {
    /// Nothing is chosen and the installer gives no default: the player must answer.
    ChoiceNeeded,
    /// More options are chosen than the group allows.
    TooManyChosen,
}

impl fmt::Display for FomodProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChoice { choice, reason } => write!(f, "choice {choice:?}: {reason}"),
            Self::GroupUnresolved {
                step,
                group,
                reason,
                ..
            } => write!(f, "{step} / {group}: {reason}"),
            Self::UnsupportedCondition { description } => {
                write!(
                    f,
                    "condition not evaluated, treated as false: {description}"
                )
            }
            Self::NotRepresentable { source, reason } => write!(f, "{source}: {reason}"),
            Self::SourceNotFound { source } => {
                write!(f, "{source}: no matching folder in the scan")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SelectedOption {
    pub step: String,
    pub group: String,
    pub option: String,
}

/// What a set of choices amounts to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Resolution {
    pub visible_steps: Vec<String>,
    pub selected: Vec<SelectedOption>,
    pub flags: BTreeMap<String, String>,
    /// Data paths relative to the scan root, in order (later overrides earlier).
    pub datapaths: Vec<PathBuf>,
    pub plugins: Vec<String>,
    pub problems: Vec<FomodProblem>,
}

struct Ctx<'a> {
    flags: BTreeMap<String, String>,
    problems: &'a mut Vec<FomodProblem>,
}

impl Ctx<'_> {
    fn eval(&mut self, c: &Condition) -> bool {
        match c {
            Condition::And { all } => all.iter().all(|c| self.eval(c)),
            Condition::Or { any } => any.iter().any(|c| self.eval(c)),
            Condition::Flag { flag, value } => self.flags.get(flag).is_some_and(|v| v == value),
            Condition::Unsupported { description } => {
                let p = FomodProblem::UnsupportedCondition {
                    description: description.clone(),
                };
                if !self.problems.contains(&p) {
                    self.problems.push(p);
                }
                false
            }
        }
    }

    fn option_type(&mut self, o: &FomodOption) -> OptionType {
        for p in &o.type_patterns {
            if self.eval(&p.condition) {
                return p.kind;
            }
        }
        o.default_type
    }
}

fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Replays the installer for `choices`. Always returns a result; anything that
/// could not be honoured is listed in `problems`.
pub fn resolve(
    info: &FomodInfo,
    choices: &[Choice],
    candidates: &[DatapathCandidate],
) -> Resolution {
    let mut res = Resolution::default();
    let mut problems = Vec::new();
    let mut ctx = Ctx {
        flags: BTreeMap::new(),
        problems: &mut problems,
    };
    let mut used = vec![false; choices.len()];
    let mut installs: Vec<FileInstall> = info.required_files.clone();

    for step in &info.steps {
        if let Some(cond) = &step.visible {
            if !ctx.eval(cond) {
                continue;
            }
        }
        res.visible_steps.push(step.name.clone());

        for group in &step.groups {
            let types: Vec<OptionType> = group.options.iter().map(|o| ctx.option_type(o)).collect();

            let mut picked: Vec<usize> = Vec::new();
            for (ci, c) in choices.iter().enumerate() {
                if c.step.as_deref().is_some_and(|s| !eq(s, &step.name))
                    || !eq(&c.group, &group.name)
                {
                    continue;
                }
                if let Some(oi) = group.options.iter().position(|o| eq(&o.name, &c.option)) {
                    used[ci] = true;
                    if types[oi] == OptionType::NotUsable {
                        ctx.problems.push(FomodProblem::InvalidChoice {
                            choice: c.to_string(),
                            reason: "this option is not usable with the current choices".into(),
                        });
                    } else if !picked.contains(&oi) {
                        picked.push(oi);
                    }
                }
            }

            if group.kind == GroupKind::SelectAll {
                picked = (0..group.options.len()).collect();
            } else {
                for (i, t) in types.iter().enumerate() {
                    if *t == OptionType::Required && !picked.contains(&i) {
                        picked.push(i);
                    }
                }
                if picked.is_empty() {
                    let recommended = types
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| **t == OptionType::Recommended);
                    match group.kind {
                        GroupKind::SelectExactlyOne | GroupKind::SelectAtMostOne => {
                            picked.extend(recommended.map(|(i, _)| i).take(1))
                        }
                        _ => picked.extend(recommended.map(|(i, _)| i)),
                    }
                }
            }
            picked.sort_unstable();

            let names = || {
                group
                    .options
                    .iter()
                    .map(|o| o.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let unresolved = |cause: Unresolved, reason: String| FomodProblem::GroupUnresolved {
                step: step.name.clone(),
                group: group.name.clone(),
                cause,
                reason,
            };
            match (group.kind, picked.len()) {
                (GroupKind::SelectExactlyOne, 0) | (GroupKind::SelectAtLeastOne, 0) => {
                    ctx.problems.push(unresolved(
                        Unresolved::ChoiceNeeded,
                        format!("a choice is needed: {}", names()),
                    ))
                }
                (GroupKind::SelectExactlyOne | GroupKind::SelectAtMostOne, n) if n > 1 => {
                    ctx.problems.push(unresolved(
                        Unresolved::TooManyChosen,
                        format!("only one may be chosen: {}", names()),
                    ))
                }
                _ => {}
            }

            for i in picked {
                let o = &group.options[i];
                for f in &o.sets_flags {
                    ctx.flags.insert(f.name.clone(), f.value.clone());
                }
                installs.extend(o.files.iter().cloned());
                res.selected.push(SelectedOption {
                    step: step.name.clone(),
                    group: group.name.clone(),
                    option: o.name.clone(),
                });
            }
        }
    }

    for (c, u) in choices.iter().zip(&used) {
        if !u {
            ctx.problems.push(FomodProblem::InvalidChoice {
                choice: c.to_string(),
                reason: "no such option in any step shown for the current choices".into(),
            });
        }
    }
    for ci in &info.conditional_installs {
        if ctx.eval(&ci.condition) {
            installs.extend(ci.files.iter().cloned());
        }
    }

    res.flags = std::mem::take(&mut ctx.flags);
    map_installs(info, installs, candidates, &mut res, &mut problems);
    res.problems = problems;
    res
}

fn rel_string(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_plugin(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PLUGIN_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Turns installs into data paths and plugins. An install maps cleanly only
/// when it puts files at the data root unchanged.
fn map_installs(
    info: &FomodInfo,
    mut installs: Vec<FileInstall>,
    candidates: &[DatapathCandidate],
    res: &mut Resolution,
    problems: &mut Vec<FomodProblem>,
) {
    // Lower priority first, so higher priority ends up later and overrides.
    installs.sort_by_key(|i| i.priority);
    let base = rel_string(&info.base);
    let join = |s: &str| {
        if base.is_empty() {
            s.to_string()
        } else {
            format!("{base}/{s}")
        }
    };
    let find = |path: &str| candidates.iter().find(|c| eq(&rel_string(&c.path), path));

    let mut shared: Vec<(PathBuf, Vec<String>)> = Vec::new();
    // Data paths reached by installing some of their folders, and which ones.
    let mut whole_folders: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for i in &installs {
        let src = join(&i.source);
        let mut unrepresentable = |reason: &str| {
            problems.push(FomodProblem::NotRepresentable {
                source: i.source.clone(),
                reason: reason.into(),
            })
        };
        if i.is_folder {
            if !i.destination.is_empty() {
                // `Part/Meshes` installed as `Meshes` is the same as using `Part` as a data path.
                let (parent, last) = src.rsplit_once('/').unwrap_or(("", src.as_str()));
                let Some(c) = eq(&i.destination, last).then(|| find(parent)).flatten() else {
                    unrepresentable(&format!("installs into the {:?} subfolder; that needs copying files, which a data path cannot express", i.destination));
                    continue;
                };
                if !res.datapaths.contains(&c.path) {
                    res.datapaths.push(c.path.clone());
                }
                match whole_folders.iter_mut().find(|(p, _)| p == &c.path) {
                    Some((_, dirs)) => dirs.push(last.to_string()),
                    None => whole_folders.push((c.path.clone(), vec![last.to_string()])),
                }
                continue;
            }
            match find(&src) {
                Some(c) => {
                    if !res.datapaths.contains(&c.path) {
                        res.datapaths.push(c.path.clone());
                    }
                    for p in &c.plugins {
                        if !res.plugins.contains(&p.name) {
                            res.plugins.push(p.name.clone());
                        }
                    }
                }
                None => problems.push(FomodProblem::SourceNotFound {
                    source: i.source.clone(),
                }),
            }
        } else {
            let (parent, name) = src.rsplit_once('/').unwrap_or(("", src.as_str()));
            let dest_name = if i.destination.is_empty() {
                name
            } else {
                i.destination.as_str()
            };
            if !eq(dest_name, name) {
                unrepresentable(
                    "is renamed or moved when installed, which a data path cannot express",
                );
                continue;
            }
            let Some(c) = find(parent) else {
                // Loose non-plugin files (readmes, screenshots) are of no interest.
                if is_plugin(name) {
                    problems.push(FomodProblem::SourceNotFound {
                        source: i.source.clone(),
                    });
                }
                continue;
            };
            if !res.datapaths.contains(&c.path) {
                res.datapaths.push(c.path.clone());
            }
            if is_plugin(name) {
                let actual = c
                    .plugins
                    .iter()
                    .find(|p| eq(&p.name, name))
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| name.to_string());
                if !res.plugins.contains(&actual) {
                    res.plugins.push(actual);
                }
            }
            if !shared.iter().any(|(p, _)| p == &c.path) {
                shared.push((c.path.clone(), Vec::new()));
            }
        }
    }
    // Likewise, a data path exposes every folder in it, not just the installed ones.
    for (path, dirs) in whole_folders {
        if let Some(c) = candidates.iter().find(|c| c.path == path) {
            let extra: Vec<&str> = c
                .engine_dirs
                .iter()
                .map(String::as_str)
                .filter(|d| !dirs.iter().any(|x| eq(x, d)))
                .collect();
            if !extra.is_empty() {
                problems.push(FomodProblem::NotRepresentable {
                    source: rel_string(&path),
                    reason: format!("its folder also contains {}, which would be used as part of this data path but was not chosen", extra.join(", ")),
                });
            }
        }
    }
    // A single file can only be offered as its whole folder; say so if the folder holds other plugins.
    for (path, _) in shared {
        if let Some(c) = candidates.iter().find(|c| c.path == path) {
            let extra: Vec<&str> = c
                .plugins
                .iter()
                .map(|p| p.name.as_str())
                .filter(|n| !res.plugins.iter().any(|p| eq(p, n)))
                .collect();
            if !extra.is_empty() {
                problems.push(FomodProblem::NotRepresentable {
                    source: rel_string(&path),
                    reason: format!("its folder also contains {}, which would be loaded as part of this data path but was not chosen", extra.join(", ")),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::PluginInfo;

    const CONFIG: &str = r#"
<config>
  <moduleName>Test Mod</moduleName>
  <requiredInstallFiles><folder source="Base" destination="" priority="0"/></requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Edition">
      <optionalFileGroups>
        <group name="Version" type="SelectExactlyOne"><plugins>
          <plugin name="Plain"><description>The plain one</description>
            <conditionFlags><flag name="Edition">Plain</flag></conditionFlags>
            <typeDescriptor><type name="Recommended"/></typeDescriptor></plugin>
          <plugin name="Deluxe"><description>More stuff</description>
            <conditionFlags><flag name="Edition">Deluxe</flag></conditionFlags>
            <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
        </plugins></group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Extras">
      <visible><dependencies operator="And"><flagDependency flag="Edition" value="Deluxe"/></dependencies></visible>
      <optionalFileGroups>
        <group name="Bonus" type="SelectAny"><plugins>
          <plugin name="Armor"><files><folder source="Extras\Armor" destination="" priority="1"/></files>
            <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
          <plugin name="Locked"><typeDescriptor><type name="NotUsable"/></typeDescriptor></plugin>
        </plugins></group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
  <conditionalFileInstalls><patterns>
    <pattern><dependencies><flagDependency flag="Edition" value="Deluxe"/></dependencies>
      <files><file source="Deluxe\Deluxe.esp" destination="Deluxe.esp"/></files></pattern>
  </patterns></conditionalFileInstalls>
</config>"#;

    #[test]
    fn reads_the_banner_and_option_pictures_relative_to_the_installer_base() {
        let xml = r#"<config><moduleName>M</moduleName><moduleImage path="Screenshots\Header.png" showImage="true"/>
          <installSteps><installStep name="S"><optionalFileGroups><group name="G" type="SelectAny"><plugins>
            <plugin name="With"><description>d</description><image path="Screenshots\one.png"/><typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
            <plugin name="Empty"><image path=" "/></plugin>
            <plugin name="Without"/>
          </plugins></group></optionalFileGroups></installStep></installSteps></config>"#;
        let info = parse(xml, Path::new("Pack")).unwrap();
        assert_eq!(
            info.module_image,
            Some(PathBuf::from("Pack/Screenshots/Header.png"))
        );
        let images: Vec<_> = info.steps[0].groups[0]
            .options
            .iter()
            .map(|o| o.image.clone())
            .collect();
        assert_eq!(
            images,
            vec![Some(PathBuf::from("Pack/Screenshots/one.png")), None, None]
        );
    }

    #[test]
    fn installers_written_before_pictures_were_read_still_load() {
        let info = parse(CONFIG, Path::new("")).unwrap();
        let mut v = serde_json::to_value(&info).unwrap();
        v.as_object_mut().unwrap().remove("module_image");
        for step in v["steps"].as_array_mut().unwrap() {
            for g in step["groups"].as_array_mut().unwrap() {
                for o in g["options"].as_array_mut().unwrap() {
                    o.as_object_mut().unwrap().remove("image");
                }
            }
        }
        assert_eq!(serde_json::from_value::<FomodInfo>(v).unwrap(), info);
    }

    fn cand(path: &str, plugins: &[&str]) -> DatapathCandidate {
        DatapathCandidate {
            path: path.into(),
            engine_dirs: vec![],
            plugins: plugins
                .iter()
                .map(|n| PluginInfo {
                    name: n.to_string(),
                    masters: vec![],
                    error: None,
                })
                .collect(),
        }
    }

    fn candidates() -> Vec<DatapathCandidate> {
        vec![
            cand("Base", &["Base.esp"]),
            cand("Extras/Armor", &["Armor.esp"]),
            cand("Deluxe", &["Deluxe.esp"]),
        ]
    }

    fn info() -> FomodInfo {
        parse(CONFIG, Path::new("")).unwrap()
    }

    fn pick(group: &str, option: &str) -> Choice {
        Choice {
            step: None,
            group: group.into(),
            option: option.into(),
        }
    }

    #[test]
    fn parses_structure() {
        let i = info();
        assert_eq!(i.module_name.as_deref(), Some("Test Mod"));
        assert_eq!(i.steps.len(), 2);
        assert_eq!(i.steps[0].groups[0].kind, GroupKind::SelectExactlyOne);
        assert_eq!(
            i.steps[0].groups[0].options[0].default_type,
            OptionType::Recommended
        );
        assert_eq!(
            i.steps[0].groups[0].options[0].sets_flags[0],
            FlagSetting {
                name: "Edition".into(),
                value: "Plain".into()
            }
        );
        assert_eq!(
            i.steps[1].visible.as_ref().unwrap().to_string(),
            "Edition = Deluxe"
        );
        assert_eq!(
            i.steps[1].groups[0].options[0].files[0].source, "Extras/Armor",
            "backslashes normalised"
        );
        assert_eq!(i.required_files[0].source, "Base");
    }

    #[test]
    fn decodes_utf16_and_utf8_boms() {
        let utf16le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("<config/>".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        let utf16be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("<config/>".encode_utf16().flat_map(|u| u.to_be_bytes()))
            .collect();
        let utf8: Vec<u8> = [0xEF, 0xBB, 0xBF]
            .into_iter()
            .chain(*b"<config/>")
            .collect();
        for b in [
            utf16le.as_slice(),
            utf16be.as_slice(),
            utf8.as_slice(),
            b"<config/>",
        ] {
            assert_eq!(decode(b), "<config/>");
            assert!(parse(&decode(b), Path::new("")).is_ok());
        }
    }

    #[test]
    fn rejects_other_xml_and_dtds() {
        assert!(matches!(
            parse("<other/>", Path::new("")),
            Err(FomodError::NotModuleConfig)
        ));
        assert!(matches!(
            parse("<!DOCTYPE x [<!ENTITY a 'b'>]><config/>", Path::new("")),
            Err(FomodError::Xml(_))
        ));
        assert!(matches!(
            parse("<config>", Path::new("")),
            Err(FomodError::Xml(_))
        ));
    }

    #[test]
    fn default_uses_recommended_and_hides_conditional_step() {
        let r = resolve(&info(), &[], &candidates());
        assert_eq!(r.visible_steps, vec!["Edition"]);
        assert_eq!(r.selected.len(), 1);
        assert_eq!(r.selected[0].option, "Plain");
        assert_eq!(r.flags["Edition"], "Plain");
        assert_eq!(r.datapaths, vec![PathBuf::from("Base")]);
        assert_eq!(r.plugins, vec!["Base.esp"]);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
    }

    #[test]
    fn choice_reveals_step_and_conditional_install_applies() {
        let r = resolve(
            &info(),
            &[pick("Version", "deluxe"), pick("Bonus", "Armor")],
            &candidates(),
        );
        assert_eq!(r.visible_steps, vec!["Edition", "Extras"]);
        assert_eq!(
            r.datapaths,
            vec![
                PathBuf::from("Base"),
                PathBuf::from("Deluxe"),
                PathBuf::from("Extras/Armor")
            ]
        );
        assert_eq!(r.plugins, vec!["Base.esp", "Deluxe.esp", "Armor.esp"]);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
    }

    #[test]
    fn choice_for_hidden_step_is_invalid() {
        let r = resolve(&info(), &[pick("Bonus", "Armor")], &candidates());
        assert!(r
            .problems
            .iter()
            .any(|p| matches!(p, FomodProblem::InvalidChoice { .. })));
        assert!(!r.datapaths.contains(&PathBuf::from("Extras/Armor")));
    }

    #[test]
    fn unknown_option_and_unusable_option_are_invalid() {
        let r = resolve(
            &info(),
            &[
                pick("Version", "Nope"),
                pick("Version", "Deluxe"),
                pick("Bonus", "Locked"),
            ],
            &candidates(),
        );
        let invalid = r
            .problems
            .iter()
            .filter(|p| matches!(p, FomodProblem::InvalidChoice { .. }))
            .count();
        assert_eq!(invalid, 2);
    }

    #[test]
    fn exactly_one_without_default_needs_a_choice() {
        let mut i = info();
        i.steps[0].groups[0].options[0].default_type = OptionType::Optional;
        let r = resolve(&i, &[], &candidates());
        assert!(r.problems.iter().any(|p| matches!(p, FomodProblem::GroupUnresolved { cause: Unresolved::ChoiceNeeded, reason, .. } if reason.contains("a choice is needed"))));
    }

    #[test]
    fn exactly_one_with_two_choices_is_unresolved() {
        let r = resolve(
            &info(),
            &[pick("Version", "Plain"), pick("Version", "Deluxe")],
            &candidates(),
        );
        assert!(r.problems.iter().any(|p| matches!(p, FomodProblem::GroupUnresolved { cause: Unresolved::TooManyChosen, reason, .. } if reason.contains("only one"))));
    }

    #[test]
    fn select_all_installs_every_option() {
        let mut i = info();
        i.steps[0].groups[0].kind = GroupKind::SelectAll;
        let r = resolve(&i, &[], &candidates());
        assert_eq!(r.selected.len(), 2);
    }

    #[test]
    fn dependency_type_patterns_follow_flags() {
        let mut i = info();
        i.steps[1].groups[0].options[1].type_patterns = vec![TypePattern {
            condition: Condition::Flag {
                flag: "Edition".into(),
                value: "Deluxe".into(),
            },
            kind: OptionType::Required,
        }];
        let r = resolve(&i, &[pick("Version", "Deluxe")], &candidates());
        assert!(
            r.selected.iter().any(|s| s.option == "Locked"),
            "required option is selected: {:?}",
            r.selected
        );
    }

    #[test]
    fn unsupported_conditions_are_false_and_reported_once() {
        let mut i = info();
        i.steps[1].visible = Some(Condition::And {
            all: vec![Condition::Unsupported {
                description: "plugin X.esp is active".into(),
            }],
        });
        i.conditional_installs[0].condition = i.steps[1].visible.clone().unwrap();
        let r = resolve(&i, &[], &candidates());
        assert_eq!(r.visible_steps, vec!["Edition"]);
        assert_eq!(
            r.problems
                .iter()
                .filter(|p| matches!(p, FomodProblem::UnsupportedCondition { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn nested_fomod_base_is_prefixed_and_matching_ignores_case() {
        let i = parse(CONFIG, Path::new("Pack")).unwrap();
        let cands = vec![cand("pack/base", &["Base.esp"])];
        let r = resolve(&i, &[], &cands);
        assert_eq!(
            r.datapaths,
            vec![PathBuf::from("pack/base")],
            "uses the casing found on disk"
        );
    }

    #[test]
    fn install_into_subfolder_or_rename_is_not_representable() {
        let mut i = info();
        i.required_files = vec![
            FileInstall {
                source: "Base".into(),
                destination: "Meshes/x".into(),
                is_folder: true,
                priority: 0,
            },
            FileInstall {
                source: "Base/Base.esp".into(),
                destination: "Other.esp".into(),
                is_folder: false,
                priority: 0,
            },
        ];
        let r = resolve(&i, &[], &candidates());
        assert_eq!(
            r.problems
                .iter()
                .filter(|p| matches!(p, FomodProblem::NotRepresentable { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn single_file_in_shared_folder_warns_about_the_others() {
        let mut i = info();
        i.required_files = vec![FileInstall {
            source: "Mix/A.esp".into(),
            destination: "A.esp".into(),
            is_folder: false,
            priority: 0,
        }];
        i.steps.clear();
        i.conditional_installs.clear();
        let cands = vec![cand("Mix", &["A.esp", "B.esp"])];
        let r = resolve(&i, &[], &cands);
        assert_eq!(r.plugins, vec!["A.esp"]);
        assert!(r.problems.iter().any(|p| matches!(p, FomodProblem::NotRepresentable { reason, .. } if reason.contains("B.esp"))));
    }

    #[test]
    fn higher_priority_installs_come_later() {
        let mut i = info();
        i.steps.clear();
        i.conditional_installs.clear();
        i.required_files = vec![
            FileInstall {
                source: "Extras/Armor".into(),
                destination: "".into(),
                is_folder: true,
                priority: 5,
            },
            FileInstall {
                source: "Base".into(),
                destination: "".into(),
                is_folder: true,
                priority: 0,
            },
        ];
        let r = resolve(&i, &[], &candidates());
        assert_eq!(
            r.datapaths,
            vec![PathBuf::from("Base"), PathBuf::from("Extras/Armor")]
        );
    }

    #[test]
    fn missing_source_is_reported() {
        let mut i = info();
        i.steps.clear();
        i.conditional_installs.clear();
        let r = resolve(&i, &[], &[]);
        assert_eq!(
            r.problems,
            vec![FomodProblem::SourceNotFound {
                source: "Base".into()
            }]
        );
    }

    #[test]
    fn choice_parsing() {
        assert_eq!(
            Choice::parse("Version=Merged").unwrap(),
            Choice {
                step: None,
                group: "Version".into(),
                option: "Merged".into()
            }
        );
        assert_eq!(
            Choice::parse("Step A/Version = Merged")
                .unwrap()
                .step
                .as_deref(),
            Some("Step A")
        );
        assert!(Choice::parse("novalue").is_none());
        assert!(Choice::parse("=x").is_none());
    }

    // Numbered-folder bundles (Project Atlas) install `Part\Meshes` as `Meshes`.
    const BUNDLE: &str = r#"
<config>
  <moduleName>Bundle</moduleName>
  <requiredInstallFiles><folder source="00 Core\Meshes" destination="Meshes" priority="0"/></requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Options">
      <optionalFileGroups>
        <group name="Textures" type="SelectExactlyOne"><plugins>
          <plugin name="Vanilla"><files><folder source="01 Textures\Textures" destination="Textures" priority="0"/></files>
            <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
          <plugin name="Both"><files>
              <folder source="02 Mushrooms\meshes" destination="MESHES" priority="0"/>
              <folder source="02 Mushrooms\sound" destination="Sound" priority="0"/></files>
            <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
          <plugin name="Renamed"><files><folder source="03 Other\Textures" destination="Skins" priority="0"/></files>
            <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
        </plugins></group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

    fn bundle_candidates() -> Vec<DatapathCandidate> {
        let dirs = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let mut v = vec![
            cand("00 Core", &[]),
            cand("01 Textures", &[]),
            cand("02 Mushrooms", &[]),
            cand("03 Other", &[]),
        ];
        v[0].engine_dirs = dirs(&["Meshes"]);
        v[1].engine_dirs = dirs(&["Textures"]);
        v[2].engine_dirs = dirs(&["meshes", "sound", "textures"]);
        v[3].engine_dirs = dirs(&["Textures"]);
        v
    }

    #[test]
    fn a_subfolder_installed_under_its_own_name_uses_its_parent_as_the_data_path() {
        let info = parse(BUNDLE, Path::new("")).unwrap();
        let r = resolve(&info, &[pick("Textures", "Vanilla")], &bundle_candidates());
        assert_eq!(
            r.datapaths,
            vec![PathBuf::from("00 Core"), PathBuf::from("01 Textures")]
        );
        assert!(r.problems.is_empty(), "{:?}", r.problems);
    }

    #[test]
    fn a_parent_with_folders_the_installer_skips_is_flagged() {
        let info = parse(BUNDLE, Path::new("")).unwrap();
        let r = resolve(&info, &[pick("Textures", "Both")], &bundle_candidates());
        assert!(r.datapaths.contains(&PathBuf::from("02 Mushrooms")));
        assert!(
            r.problems.iter().any(|p| matches!(
                p,
                FomodProblem::NotRepresentable { source, reason }
                    if source == "02 Mushrooms" && reason.contains("textures")
            )),
            "{:?}",
            r.problems
        );
    }

    #[test]
    fn a_subfolder_installed_under_another_name_still_cannot_be_a_data_path() {
        let info = parse(BUNDLE, Path::new("")).unwrap();
        let r = resolve(&info, &[pick("Textures", "Renamed")], &bundle_candidates());
        assert!(!r.datapaths.contains(&PathBuf::from("03 Other")));
        assert!(r.problems.iter().any(|p| matches!(
            p,
            FomodProblem::NotRepresentable { reason, .. } if reason.contains("Skins")
        )));
    }
}
