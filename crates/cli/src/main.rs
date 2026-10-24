use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use owl_core::apply;
use owl_core::cfg::{self, ModEntry};
use owl_core::fomod;
use owl_core::library;
use owl_core::manifest::Manifest;
use owl_core::order_fix;
use owl_core::profile::{self, LoadedMod, Profile};
use owl_core::scan::{self, ScanOptions};
use owl_core::validate::{self, ValidationIssue};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Parser)]
#[command(
    name = "owl",
    about = "Set up OpenMW mods without hand-editing openmw.cfg"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan a mod directory and report what was found
    Scan {
        dir: PathBuf,
        /// Emit JSON instead of human-readable text
        #[arg(long)]
        json: bool,
        /// How many directory levels below DIR to search
        #[arg(long, default_value_t = scan::DEFAULT_MAX_DEPTH)]
        max_depth: usize,
    },
    /// Say whether a folder is one mod or a folder of many, and list the mods in it
    Library {
        dir: PathBuf,
        /// Emit JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },
    /// Scan a mod directory and write a manifest for it
    Init(InitArgs),
    /// Check a profile's load order for problems (changes nothing)
    Check(ProfileArgs),
    /// Reorder the profile so every plugin loads after the plugins it needs
    FixOrder {
        #[command(flatten)]
        common: ProfileArgs,
        /// Show what would move without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Write the profile's mods into openmw.cfg
    Apply {
        #[command(flatten)]
        common: ProfileArgs,
        /// Show what would change without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Write even if the check found problems
        #[arg(long)]
        force: bool,
    },
}

#[derive(Args)]
struct InitArgs {
    dir: PathBuf,
    /// Mod name (default: the folder's name)
    #[arg(long)]
    name: Option<String>,
    /// Where to write the manifest (default: ./<name>.owl.json)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Also add the mod to the end of this profile (created if missing)
    #[arg(long)]
    profile: Option<PathBuf>,
    /// Data path to use, relative to DIR; repeat for several. Needed when the scan finds more than one candidate
    #[arg(long = "datapath")]
    datapaths: Vec<PathBuf>,
    /// FOMOD choice as "Group=Option" or "Step/Group=Option"; repeat for several. Run `owl scan` to see them
    #[arg(long = "option")]
    options: Vec<String>,
    /// Plugin to enable; repeat for several. Default: every plugin in the chosen data paths
    #[arg(long = "plugin")]
    plugins: Vec<String>,
    /// Overwrite the manifest if it already exists
    #[arg(long)]
    force: bool,
    #[arg(long, default_value_t = scan::DEFAULT_MAX_DEPTH)]
    max_depth: usize,
}

#[derive(Args)]
struct ProfileArgs {
    /// Profile file listing your mods in load order (default: the one the app uses)
    #[arg(long)]
    profile: Option<PathBuf>,
    /// openmw.cfg to use (default: found automatically, see docs)
    #[arg(long)]
    cfg: Option<PathBuf>,
    /// Emit JSON (check only)
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Scan {
            dir,
            json,
            max_depth,
        } => cmd_scan(&dir, json, max_depth),
        Command::Init(args) => cmd_init(args),
        Command::Check(args) => cmd_check(&args),
        Command::Apply {
            common,
            dry_run,
            force,
        } => cmd_apply(&common, dry_run, force),
        Command::Library { dir, json } => cmd_library(&dir, json),
        Command::FixOrder { common, dry_run } => cmd_fix_order(&common, dry_run),
    }
}

fn cmd_scan(dir: &Path, json: bool, max_depth: usize) -> Result<ExitCode> {
    let report = scan::scan(dir, &ScanOptions { max_depth })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{} datapath candidate(s)", report.datapath_candidates.len());
        println!("{} plugin(s)", report.plugins().len());
        if let Some(info) = &report.fomod {
            print_fomod(info, &report);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_library(dir: &Path, json: bool) -> Result<ExitCode> {
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()).into());
    }
    let kind = library::classify(dir);
    let listing = (kind == library::FolderKind::Library).then(|| library::read_library(dir, &[]));
    if json {
        let value = serde_json::json!({ "kind": kind, "library": listing });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(ExitCode::SUCCESS);
    }
    match listing {
        None => println!("{} is one mod. Use `owl scan` or `owl init` on it.", dir.display()),
        Some(lib) => {
            println!("{} holds several mods:\n", dir.display());
            for e in &lib.entries {
                let what = match e.kind {
                    library::EntryKind::Mod => "mod",
                    library::EntryKind::Archive => "archive (extract it first)",
                    library::EntryKind::Other => "no mod found",
                };
                println!("  {:<48} {what}", e.name);
            }
            if lib.truncated {
                println!("\n(only the first part of the folder is listed)");
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// "Here's what we found" for a FOMOD installer.
fn print_fomod(info: &fomod::FomodInfo, report: &scan::ScanReport) {
    let title = info.module_name.as_deref().unwrap_or("(unnamed)");
    println!("\nThis mod ships an installer (FOMOD): {title}");
    let defaults = fomod::resolve(info, &[], &report.datapath_candidates);
    for (n, step) in info.steps.iter().enumerate() {
        let shown = match &step.visible {
            Some(c) => format!("  [only shown when {c}]"),
            None => String::new(),
        };
        println!("\n  Step {}: {}{shown}", n + 1, step.name);
        for g in &step.groups {
            println!("    {} ({})", g.name, g.kind.describe());
            for o in &g.options {
                let default = defaults
                    .selected
                    .iter()
                    .any(|s| s.step == step.name && s.group == g.name && s.option == o.name);
                let mark = if default { "*" } else { " " };
                // In an "all are installed" group the type is not a choice.
                let note = match o.default_type {
                    _ if g.kind == fomod::GroupKind::SelectAll => "",
                    fomod::OptionType::Recommended => " [recommended]",
                    fomod::OptionType::Required => " [required]",
                    fomod::OptionType::NotUsable => " [unavailable]",
                    _ => "",
                };
                println!("      {mark} {}{note}", o.name);
                if let Some(line) = o
                    .description
                    .lines()
                    .map(str::trim)
                    .find(|l| l.chars().any(char::is_alphabetic))
                {
                    let line: String = line.chars().take(90).collect();
                    println!("          {line}");
                }
            }
        }
    }
    println!("\n  (* = what the installer picks by default)");
    println!("  Choose with: owl init <DIR> --option \"Group=Option\" ...");
}

fn cmd_init(a: InitArgs) -> Result<ExitCode> {
    let root = a
        .dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", a.dir.display()))?;
    let report = scan::scan(
        &root,
        &ScanOptions {
            max_depth: a.max_depth,
        },
    )?;
    let name = a
        .name
        .or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "mod".into());
    let mut manifest = Manifest::from_scan(&name, &report);

    let choices = a
        .options
        .iter()
        .map(|o| {
            fomod::Choice::parse(o).ok_or_else(|| {
                format!("--option {o:?}: expected \"Group=Option\" or \"Step/Group=Option\"")
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut fomod_resolution = None;
    if !choices.is_empty() {
        if !a.datapaths.is_empty() || !a.plugins.is_empty() {
            return Err("--option cannot be combined with --datapath or --plugin".into());
        }
        let res = manifest
            .choose_fomod(choices)
            .ok_or("this mod has no FOMOD installer; use --datapath instead")?;
        let invalid: Vec<String> = res
            .problems
            .iter()
            .filter(|p| matches!(p, fomod::FomodProblem::InvalidChoice { .. }))
            .map(|p| p.to_string())
            .collect();
        if !invalid.is_empty() {
            return Err(format!(
                "{}\nrun `owl scan` to see the available choices",
                invalid.join("\n")
            )
            .into());
        }
        fomod_resolution = Some(res);
    }
    if fomod_resolution.is_some() {
        // decisions already set from the FOMOD choices
    } else if !a.datapaths.is_empty() {
        let issues = manifest.choose_datapaths(a.datapaths.clone(), a.plugins.clone());
        if !issues.is_empty() {
            let msgs: Vec<String> = issues
                .into_iter()
                .map(|issue| {
                    ValidationIssue::StaleDecision {
                        mod_name: name.clone(),
                        issue,
                    }
                    .to_string()
                })
                .collect();
            return Err(format!(
                "{}\nrun `owl scan` to see the available choices",
                msgs.join("\n")
            )
            .into());
        }
    } else if !a.plugins.is_empty() && manifest.decisions.datapaths.is_empty() {
        return Err("--plugin needs --datapath when the scan found more than one candidate".into());
    }

    let out = a
        .out
        .unwrap_or_else(|| PathBuf::from(format!("{}.owl.json", file_safe(&name))));
    if out.exists() && !a.force {
        return Err(format!("{} already exists; use --force to overwrite", out.display()).into());
    }
    profile::write_manifest(&out, &manifest)?;
    println!("wrote {}", out.display());

    if let Some(res) = &fomod_resolution {
        println!("installer choices:");
        for sel in &res.selected {
            println!("  {} / {}: {}", sel.step, sel.group, sel.option);
        }
        for p in &res.problems {
            println!("  note: {p}");
        }
    }
    if manifest.decisions.datapaths.is_empty()
        && manifest.derived.fomod.is_some()
        && fomod_resolution.is_none()
    {
        println!("\nThis mod has an installer; run `owl scan` to see its options, then re-run with --option and --force.");
    } else if manifest.decisions.datapaths.is_empty() {
        println!(
            "\n{} possible data paths; choose with --datapath and re-run with --force:",
            manifest.derived.datapath_candidates.len()
        );
        for c in &manifest.derived.datapath_candidates {
            let shown = if c.path.as_os_str().is_empty() {
                ".".into()
            } else {
                c.path.display().to_string()
            };
            let plugins: Vec<_> = c.plugins.iter().map(|p| p.name.as_str()).collect();
            println!("  {shown}  [{}]", plugins.join(", "));
        }
    } else {
        println!(
            "data paths: {}",
            display_list(
                manifest
                    .decisions
                    .datapaths
                    .iter()
                    .map(|p| p.display().to_string())
            )
        );
        println!(
            "plugins:    {}",
            display_list(manifest.decisions.enabled_plugins.iter().cloned())
        );
    }

    if let Some(profile_path) = a.profile {
        let mut p = if profile_path.exists() {
            profile::read_profile(&profile_path)?
        } else {
            Profile::default()
        };
        let manifest_abs = out.canonicalize()?;
        if p.add(manifest_abs, root) {
            profile::write_profile(&profile_path, &p)?;
            println!("added to {}", profile_path.display());
        } else {
            println!("already in {}", profile_path.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `--profile`, or the profile the desktop app keeps, so the two share one list.
fn profile_path(a: &ProfileArgs) -> Result<PathBuf> {
    if let Some(p) = &a.profile {
        return Ok(p.clone());
    }
    let dir =
        owl_core::locate::store_dir(owl_core::locate::Os::current(), &|k| std::env::var_os(k))?;
    let path = profile::store_profile_path(&dir);
    if !path.exists() {
        return Err(format!(
            "no mods added yet ({} does not exist). Add one in the app, or run `owl init <DIR> --profile <FILE>` and pass --profile",
            path.display()
        )
        .into());
    }
    Ok(path)
}

fn cmd_check(a: &ProfileArgs) -> Result<ExitCode> {
    let (_cfg_path, base) = read_cfg(a.cfg.clone(), false)?;
    let profile_path = profile_path(a)?;
    let unmanaged = cfg::unmanaged_text(&base, &profile::applied_lines(&profile_path))?;
    let mods = profile::load_mods(&profile_path)?;
    let report = validate::check(&unmanaged, &entries_of(&mods));

    if a.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for m in &report.mods {
            println!("{}", m.mod_name);
            if m.requirements.is_empty() {
                println!("  needs: nothing");
            }
            for r in &m.requirements {
                let owl_core::manifest::RequirementKind::Master { name } = &r.kind;
                println!(
                    "  needs {name}: {:?} (for {})",
                    r.status,
                    r.needed_by.join(", ")
                );
            }
        }
        if report.issues.is_empty() {
            println!("\nno problems found");
        } else {
            println!("\n{} problem(s):", report.issues.len());
            for i in &report.issues {
                println!("  - {i}");
            }
        }
    }
    Ok(if report.issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn cmd_fix_order(a: &ProfileArgs, dry_run: bool) -> Result<ExitCode> {
    // The player's own openmw.cfg is optional here: it only tells us which plugins are already loaded.
    let path = profile_path(a)?;
    let owned = profile::applied_lines(&path);
    let base = read_cfg(a.cfg.clone(), true)
        .ok()
        .and_then(|(_, text)| cfg::unmanaged_text(&text, &owned).ok())
        .unwrap_or_default();
    let fix = if dry_run {
        order_fix::plan_fix(&base, &profile::load_mods(&path)?)
    } else {
        profile::fix_order(&path, &base)?
    };

    if !fix.cyclic_mods.is_empty() {
        eprintln!(
            "these mods need each other in a loop, so no order of whole mods works: {}",
            fix.cyclic_mods.join(", ")
        );
        return Ok(ExitCode::FAILURE);
    }
    if !fix.changed {
        println!("the order is already fine");
        return Ok(ExitCode::SUCCESS);
    }
    for m in fix.moves.iter().filter(|m| m.to < m.from) {
        println!(
            "{} moves up: position {} -> {}",
            m.name,
            m.from + 1,
            m.to + 1
        );
    }
    for r in &fix.plugin_reorders {
        println!(
            "{}: plugins {} -> {}",
            r.mod_name,
            r.before.join(", "),
            r.after.join(", ")
        );
    }
    println!("\nnew order:");
    for (i, name) in fix.order.iter().enumerate() {
        println!("  {}. {name}", i + 1);
    }
    println!(
        "{}",
        if dry_run {
            "\ndry run: nothing changed"
        } else {
            "\nyour list is updated; run `owl apply` to use it in OpenMW"
        }
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_apply(a: &ProfileArgs, dry_run: bool, force: bool) -> Result<ExitCode> {
    // A missing explicit --cfg is fine for apply (it is created); a missing auto-detected one is an error.
    let (cfg_path, base) = read_cfg(a.cfg.clone(), true)?;
    let profile_path = profile_path(a)?;
    let mods = profile::load_mods(&profile_path)?;
    let owned = profile::applied_lines(&profile_path);
    let (plan, _) = apply::plan(&cfg_path, &base, &mods, &owned)?;

    if !plan.issues.is_empty() {
        eprintln!("{} problem(s):", plan.issues.len());
        for i in &plan.issues {
            eprintln!("  - {i}");
        }
        if !force {
            eprintln!("\nnothing written; fix these or use --force");
            return Ok(ExitCode::FAILURE);
        }
    }
    for line in &plan.skipped_duplicates {
        println!("already in your config, left alone: {line}");
    }
    for line in &plan.kept {
        println!("in Owl's section but not written by Owl, left in place: {line}");
    }

    if dry_run {
        println!("{}", cfg_path.display());
        if !plan.changed {
            println!("no changes");
        }
        for l in &plan.removed {
            println!("- {l}");
        }
        for l in &plan.added {
            println!("+ {l}");
        }
        println!("\ndry run: nothing written");
        return Ok(ExitCode::SUCCESS);
    }

    // The fingerprint ties the write to exactly what was just planned.
    let outcome = apply::apply(&cfg_path, &mods, &profile_path, Some(&plan.fingerprint), force)?;
    if !outcome.changed {
        println!("{} already up to date", cfg_path.display());
    } else {
        println!("updated {}", cfg_path.display());
        if let Some(b) = outcome.backup {
            println!("previous version saved as {b}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Resolves openmw.cfg and reads it. `explicit_may_be_missing`: a missing
/// `--cfg` file reads as empty so `apply` can create it.
fn read_cfg(explicit: Option<PathBuf>, explicit_may_be_missing: bool) -> Result<(PathBuf, String)> {
    let was_explicit = explicit.is_some();
    let path = owl_core::locate::resolve(explicit, &|k| std::env::var_os(k), &|p| p.exists())?;
    match fs::read_to_string(&path) {
        Ok(t) => Ok((path, t)),
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && was_explicit
                && explicit_may_be_missing =>
        {
            Ok((path, String::new()))
        }
        Err(e) => Err(format!("{}: {e}", path.display()).into()),
    }
}

fn entries_of(mods: &[LoadedMod]) -> Vec<ModEntry<'_>> {
    mods.iter()
        .map(|m| ModEntry {
            manifest: &m.manifest,
            root: &m.root,
        })
        .collect()
}

fn file_safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn display_list(items: impl Iterator<Item = String>) -> String {
    let v: Vec<String> = items
        .map(|s| if s.is_empty() { ".".into() } else { s })
        .collect();
    v.join(", ")
}
