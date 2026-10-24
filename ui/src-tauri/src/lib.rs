//! Thin Tauri commands over `owl-core`. Anything that decides something lives
//! in the core; these only translate between the page and the core, and turn
//! errors into strings the page can show.

use std::path::PathBuf;

use owl_core::apply::{self, ApplyOutcome, ApplyPlan};
use owl_core::cfg;
use owl_core::fomod::Choice;
use owl_core::images;
use owl_core::library::{self, FolderKind};
use owl_core::locate;
use owl_core::manifest::{self, Manifest, Preview};
use owl_core::order_fix::{self, OrderFix};
use owl_core::profile::{self, LoadedMod, ProfileView};
use owl_core::scan::{self, ScanOptions, ScanReport};

type CmdResult<T> = Result<T, String>;

/// Where the player's `openmw.cfg` is, or why it could not be found.
#[tauri::command]
fn locate_config() -> CmdResult<String> {
    locate::resolve(None, &|k| std::env::var_os(k), &|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan_folder(path: String) -> CmdResult<ScanReport> {
    // Walking a large folder blocks; keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        scan::scan(&PathBuf::from(path), &ScanOptions::default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The player's own `openmw.cfg` text, without the lines Owl wrote. Empty if
/// it cannot be read, so screens still work (requirements then say "can't tell").
fn unmanaged_cfg(cfg_path: Option<&str>) -> Option<String> {
    let text = std::fs::read_to_string(cfg_path?).ok()?;
    let owned = store_dir()
        .map(|d| profile::applied_lines(&profile::store_profile_path(&d)))
        .unwrap_or_default();
    cfg::unmanaged_text(&text, &owned).ok()
}

/// Plugins the player already lists in `openmw.cfg` (not the part Owl manages).
fn installed_plugins(cfg_path: Option<&str>) -> Option<Vec<String>> {
    unmanaged_cfg(cfg_path).map(|t| cfg::content_entries(&t))
}

/// Where the player's profile and mod manifests live; shared with the command line.
fn store_dir() -> CmdResult<PathBuf> {
    locate::store_dir(locate::Os::current(), &|k| std::env::var_os(k)).map_err(|e| e.to_string())
}

#[tauri::command]
fn preview_mod(
    report: ScanReport,
    name: String,
    fomod_choices: Vec<Choice>,
    datapaths: Vec<PathBuf>,
    plugins: Vec<String>,
    cfg_path: Option<String>,
) -> Preview {
    let available = installed_plugins(cfg_path.as_deref());
    manifest::preview(
        &report,
        &name,
        fomod_choices,
        datapaths,
        plugins,
        available.as_deref(),
    )
}

/// Whether the folder the player chose is one mod or a folder of many.
#[tauri::command]
async fn classify_folder(path: String) -> CmdResult<FolderKind> {
    // Looks a few levels into each folder; keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(move || library::classify(&PathBuf::from(path)))
        .await
        .map_err(|e| e.to_string())
}

/// Remembers a place to look for mods: a folder of many mods (`library`) or
/// the folder of one mod that is not on the list yet (`mod`).
#[tauri::command]
fn add_source(path: String, kind: FolderKind) -> CmdResult<bool> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }
    profile::add_source_to_store(&store_dir()?, &path, kind).map_err(|e| e.to_string())
}

/// Forgets a source. Mods already added from it stay on the list, and the
/// folder is untouched.
#[tauri::command]
fn remove_source(path: String) -> CmdResult<()> {
    profile::remove_source_from_store(&store_dir()?, &PathBuf::from(path))
        .map_err(|e| e.to_string())
}

/// Like `preview_mod`, for a mod that is already on the list: starts from its
/// stored manifest and replays the installer (or the folder choice) for the
/// given answers. Changes nothing; saving goes through `add_to_profile`.
#[tauri::command]
fn preview_stored_mod(
    manifest: Manifest,
    fomod_choices: Vec<Choice>,
    datapaths: Vec<PathBuf>,
    plugins: Vec<String>,
    cfg_path: Option<String>,
) -> Preview {
    let available = installed_plugins(cfg_path.as_deref());
    manifest::preview_stored(
        &manifest,
        fomod_choices,
        datapaths,
        plugins,
        available.as_deref(),
    )
}

/// Path of the profile file, so the player can point the command line at it.
#[tauri::command]
fn profile_path() -> CmdResult<String> {
    Ok(profile::store_profile_path(&store_dir()?)
        .to_string_lossy()
        .into_owned())
}

/// The player's mods in load order, with the check of the whole setup.
#[tauri::command]
fn load_profile(cfg_path: Option<String>) -> CmdResult<ProfileView> {
    let base = unmanaged_cfg(cfg_path.as_deref()).unwrap_or_default();
    profile::load_view(&store_dir()?, &base).map_err(|e| e.to_string())
}

/// Adds a mod, or updates it if its folder is already in the profile.
#[tauri::command]
fn add_to_profile(manifest: Manifest, root: String) -> CmdResult<()> {
    profile::add_to_store(&store_dir()?, &manifest, &PathBuf::from(root)).map_err(|e| e.to_string())
}

/// A picture the mod's author put in the installer (its banner or an option's
/// picture), as a `data:` address. `root` is the mod's folder, `path` the
/// picture inside it; the core refuses anything outside the folder, links, and
/// files that are not pictures, so this can only ever return an image.
#[tauri::command]
async fn read_mod_image(root: String, path: String) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        images::read_image(&PathBuf::from(root), &PathBuf::from(path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reads a mod's folder again (for example after the mod was updated, or to
/// pick up the author's notes). Returns the player's choices that no longer
/// match the folder.
#[tauri::command]
async fn rescan_mod(index: usize) -> CmdResult<Vec<manifest::DecisionIssue>> {
    let dir = store_dir()?;
    // Walking a large folder blocks; keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        profile::rescan_in_store(&dir, index).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn move_in_profile(from: usize, to: usize) -> CmdResult<()> {
    profile::move_in_store(&store_dir()?, from, to).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_from_profile(index: usize) -> CmdResult<()> {
    profile::remove_from_store(&store_dir()?, index).map_err(|e| e.to_string())
}

/// What fixing the load order would change. Changes nothing.
#[tauri::command]
fn plan_order_fix(cfg_path: Option<String>) -> CmdResult<OrderFix> {
    let base = unmanaged_cfg(cfg_path.as_deref()).unwrap_or_default();
    Ok(order_fix::plan_fix(&base, &stored_mods()?))
}

/// Reorders the player's list so every plugin loads after its masters.
#[tauri::command]
fn fix_order(cfg_path: Option<String>) -> CmdResult<OrderFix> {
    let base = unmanaged_cfg(cfg_path.as_deref()).unwrap_or_default();
    profile::fix_order_in_store(&store_dir()?, &base).map_err(|e| e.to_string())
}

/// The mods on the player's list, in load order (none if nothing is added yet).
fn stored_mods() -> CmdResult<Vec<LoadedMod>> {
    let path = profile::store_profile_path(&store_dir()?);
    if !path.exists() {
        return Ok(Vec::new());
    }
    profile::load_mods(&path).map_err(|e| e.to_string())
}

/// The `openmw.cfg` to write. Found here, never taken from the page, so the
/// page cannot point a write at some other file.
fn target_cfg() -> CmdResult<PathBuf> {
    locate::resolve(None, &|k| std::env::var_os(k), &|p| p.exists()).map_err(|e| e.to_string())
}

/// What applying would do. Writes nothing.
#[tauri::command]
fn plan_apply() -> CmdResult<ApplyPlan> {
    let cfg_path = target_cfg()?;
    let base = apply::read_cfg(&cfg_path).map_err(|e| e.to_string())?;
    let owned = profile::applied_lines(&profile::store_profile_path(&store_dir()?));
    apply::plan(&cfg_path, &base, &stored_mods()?, &owned)
        .map(|(plan, _)| plan)
        .map_err(|e| e.to_string())
}

/// Writes the settings, but only if they would come out exactly as reviewed.
#[tauri::command]
fn apply_changes(fingerprint: String, force: bool) -> CmdResult<ApplyOutcome> {
    let profile_path = profile::store_profile_path(&store_dir()?);
    apply::apply(
        &target_cfg()?,
        &stored_mods()?,
        &profile_path,
        Some(&fingerprint),
        force,
    )
    .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            locate_config,
            scan_folder,
            preview_mod,
            preview_stored_mod,
            profile_path,
            load_profile,
            add_to_profile,
            rescan_mod,
            classify_folder,
            add_source,
            remove_source,
            read_mod_image,
            move_in_profile,
            remove_from_profile,
            plan_apply,
            apply_changes,
            plan_order_fix,
            fix_order
        ])
        .run(tauri::generate_context!())
        .expect("error while running Owl");
}
