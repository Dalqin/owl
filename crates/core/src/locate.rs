//! Finds the user's `openmw.cfg`, for any front end. Environment and filesystem are passed in so
//! the lookup is testable.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

const FLATPAK_ID: &str = "org.openmw.OpenMW";

#[derive(Debug)]
pub enum ConfigError {
    Ambiguous {
        native: PathBuf,
        flatpak: PathBuf,
    },
    NotFound {
        tried: Vec<PathBuf>,
    },
    NoHome,
    /// No per-user data folder could be worked out (`HOME` / `APPDATA` unset).
    NoStoreHome,
    /// The player chose a file that is not an `openmw.cfg`.
    NotOpenmwCfg(PathBuf),
    /// The choice could not be saved or removed.
    Save(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ambiguous { native, flatpak } => write!(
                f,
                "found both {} and {}; pass --cfg to choose one",
                native.display(),
                flatpak.display()
            ),
            Self::NotFound { tried } => {
                let list: Vec<_> = tried.iter().map(|p| p.display().to_string()).collect();
                write!(
                    f,
                    "no openmw.cfg found (looked in: {}). Run OpenMW once to create it, or pass --cfg",
                    list.join(", ")
                )
            }
            Self::NoStoreHome => write!(
                f,
                "cannot locate your user data folder (HOME or APPDATA is not set); pass --profile"
            ),
            Self::NotOpenmwCfg(p) => write!(
                f,
                "{} is not an openmw.cfg file; choose the file named openmw.cfg",
                p.display()
            ),
            Self::Save(e) => write!(f, "could not save your choice of openmw.cfg: {e}"),
            Self::NoHome => write!(
                f,
                "cannot locate your home folder (HOME is not set); pass --cfg"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Mac,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

/// Where Owl keeps the player's own data (their profile and mod manifests), so
/// the app and the command line find the same files without being told:
/// Linux `$XDG_DATA_HOME/owl` (default `~/.local/share/owl`), macOS
/// `~/Library/Application Support/owl`, Windows `%APPDATA%\owl`.
pub fn store_dir(os: Os, env: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, ConfigError> {
    let home = env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    let base = match os {
        Os::Windows => env("APPDATA").filter(|h| !h.is_empty()).map(PathBuf::from),
        Os::Mac => home.map(|h| h.join("Library/Application Support")),
        Os::Linux => env("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.has_root())
            .or_else(|| home.map(|h| h.join(".local/share"))),
    };
    base.map(|b| b.join("owl")).ok_or(ConfigError::NoStoreHome)
}

const CHOSEN_CFG_FILE: &str = "openmw-cfg-path";

/// The `openmw.cfg` the player picked by hand, if they did and it still exists.
/// Kept in Owl's own folder (`dir`, see [`store_dir`]); a missing or stale choice
/// is ignored so the normal lookup takes over.
pub fn chosen_config(dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dir.join(CHOSEN_CFG_FILE)).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_file().then_some(path)
}

/// Remembers a hand-picked `openmw.cfg`. Only an existing file named
/// `openmw.cfg` is accepted, so a bad choice cannot send a write elsewhere.
pub fn choose_config(dir: &Path, path: &Path) -> Result<(), ConfigError> {
    let named_right = path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("openmw.cfg"));
    if !named_right || !path.is_file() {
        return Err(ConfigError::NotOpenmwCfg(path.to_path_buf()));
    }
    let save = || -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(CHOSEN_CFG_FILE), path.to_string_lossy().as_bytes())
    };
    save().map_err(|e| ConfigError::Save(e.to_string()))
}

/// Forgets the hand-picked choice, going back to finding it automatically.
pub fn forget_chosen_config(dir: &Path) -> Result<(), ConfigError> {
    match std::fs::remove_file(dir.join(CHOSEN_CFG_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ConfigError::Save(e.to_string())),
    }
}

/// Where OpenMW keeps `openmw.cfg` on Windows and macOS, best first. These are
/// OpenMW's documented defaults; the Windows ones have not been tried on a real
/// Windows machine. A "Documents" folder redirected somewhere else (for example
/// to OneDrive) is covered only for the usual OneDrive spot; otherwise the player
/// picks the file by hand.
fn desktop_candidates(os: Os, env: &dyn Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    match os {
        Os::Windows => {
            let Some(profile) = var("USERPROFILE").or_else(|| var("HOME")) else {
                return Vec::new();
            };
            let tail = Path::new("My Games").join("OpenMW").join("openmw.cfg");
            vec![
                profile.join("Documents").join(&tail),
                profile.join("OneDrive").join("Documents").join(&tail),
            ]
        }
        Os::Mac => var("HOME")
            .map(|h| h.join("Library/Preferences/openmw/openmw.cfg"))
            .into_iter()
            .collect(),
        Os::Linux => Vec::new(),
    }
}

/// Order: explicit `--cfg`, then the native location (`$XDG_CONFIG_HOME` or
/// `~/.config`), then the Flatpak location. If both native and Flatpak exist
/// it refuses to guess. On Windows and macOS the usual folders for those
/// systems are tried instead.
pub fn resolve(
    explicit: Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<OsString>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, ConfigError> {
    resolve_for(Os::current(), explicit, env, exists)
}

/// [`resolve`] for a given system, so each can be tested on any machine.
pub fn resolve_for(
    os: Os,
    explicit: Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<OsString>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, ConfigError> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if os != Os::Linux {
        let tried = desktop_candidates(os, env);
        return match tried.iter().find(|p| exists(p)) {
            Some(found) => Ok(found.clone()),
            None if tried.is_empty() => Err(ConfigError::NoHome),
            None => Err(ConfigError::NotFound { tried }),
        };
    }
    let home = env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    // Inside a Flatpak sandbox `XDG_CONFIG_HOME` points at the app's own private
    // folder, not where OpenMW keeps its config, so only `~/.config` is meaningful.
    let sandboxed = env("FLATPAK_ID").is_some();
    let xdg = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.has_root() && !sandboxed);

    let native_dir = match (&xdg, &home) {
        (Some(x), _) => x.clone(),
        (None, Some(h)) => h.join(".config"),
        (None, None) => return Err(ConfigError::NoHome),
    };
    let native = native_dir.join("openmw").join("openmw.cfg");
    let flatpak = home.map(|h| {
        h.join(".var/app")
            .join(FLATPAK_ID)
            .join("config/openmw/openmw.cfg")
    });

    match (exists(&native), flatpak.as_ref().filter(|p| exists(p))) {
        (true, Some(fp)) => Err(ConfigError::Ambiguous {
            native,
            flatpak: fp.clone(),
        }),
        (true, None) => Ok(native),
        (false, Some(fp)) => Ok(fp.clone()),
        (false, None) => {
            let mut tried = vec![native];
            tried.extend(flatpak);
            Err(ConfigError::NotFound { tried })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| OsString::from(v))
        }
    }

    fn exists_in<'a>(paths: &'a [&'a str]) -> impl Fn(&Path) -> bool + 'a {
        move |p| paths.iter().any(|x| Path::new(x) == p)
    }

    const NATIVE: &str = "/home/u/.config/openmw/openmw.cfg";
    const FLATPAK: &str = "/home/u/.var/app/org.openmw.OpenMW/config/openmw/openmw.cfg";

    #[test]
    fn windows_looks_in_documents_my_games() {
        let env = env_of(&[("USERPROFILE", "C:\\Users\\u")]);
        let want = Path::new("C:\\Users\\u").join("Documents/My Games/OpenMW/openmw.cfg");
        let found = want.clone();
        let got = resolve_for(Os::Windows, None, &env, &move |p| p == found).unwrap();
        assert_eq!(got, want);
        let err = resolve_for(Os::Windows, None, &env, &|_| false).unwrap_err();
        assert!(matches!(err, ConfigError::NotFound { tried } if tried.len() == 2));
        let err = resolve_for(Os::Windows, None, &env_of(&[]), &|_| false).unwrap_err();
        assert!(matches!(err, ConfigError::NoHome));
    }

    #[test]
    fn macos_looks_in_library_preferences() {
        let got = resolve_for(
            Os::Mac,
            None,
            &env_of(&[("HOME", "/Users/u")]),
            &exists_in(&["/Users/u/Library/Preferences/openmw/openmw.cfg"]),
        )
        .unwrap();
        assert_eq!(
            got,
            PathBuf::from("/Users/u/Library/Preferences/openmw/openmw.cfg")
        );
    }

    #[test]
    fn a_chosen_config_is_remembered_and_can_be_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("Some Place").join("OpenMW.cfg");
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, "").unwrap();
        let store = dir.path().join("store");
        assert_eq!(chosen_config(&store), None);
        choose_config(&store, &cfg).unwrap();
        assert_eq!(chosen_config(&store), Some(cfg.clone()));
        std::fs::remove_file(&cfg).unwrap();
        assert_eq!(chosen_config(&store), None, "a vanished file is ignored");
        forget_chosen_config(&store).unwrap();
        forget_chosen_config(&store).unwrap();
    }

    #[test]
    fn only_an_existing_openmw_cfg_can_be_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("settings.cfg");
        std::fs::write(&other, "").unwrap();
        let missing = dir.path().join("openmw.cfg");
        for bad in [other, missing] {
            assert!(matches!(
                choose_config(dir.path(), &bad),
                Err(ConfigError::NotOpenmwCfg(_))
            ));
        }
        assert_eq!(chosen_config(dir.path()), None);
    }

    #[test]
    fn explicit_wins_even_if_missing() {
        let got = resolve(Some("/x/y.cfg".into()), &env_of(&[]), &exists_in(&[])).unwrap();
        assert_eq!(got, PathBuf::from("/x/y.cfg"));
    }

    #[test]
    fn native_default_uses_home_dot_config() {
        let got = resolve(None, &env_of(&[("HOME", "/home/u")]), &exists_in(&[NATIVE])).unwrap();
        assert_eq!(got, PathBuf::from(NATIVE));
    }

    #[test]
    fn xdg_config_home_overrides_default() {
        let env = env_of(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/cfg")]);
        let got = resolve(None, &env, &exists_in(&["/cfg/openmw/openmw.cfg"])).unwrap();
        assert_eq!(got, PathBuf::from("/cfg/openmw/openmw.cfg"));
    }

    #[test]
    fn inside_a_flatpak_xdg_config_home_is_the_sandbox_and_ignored() {
        let env = env_of(&[
            ("HOME", "/home/u"),
            ("FLATPAK_ID", "app.owl.openmw"),
            ("XDG_CONFIG_HOME", "/home/u/.var/app/app.owl.openmw/config"),
        ]);
        let got = resolve(None, &env, &exists_in(&[NATIVE])).unwrap();
        assert_eq!(got, PathBuf::from(NATIVE));
    }

    #[test]
    fn relative_xdg_is_ignored_per_spec() {
        let env = env_of(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "rel")]);
        let got = resolve(None, &env, &exists_in(&[NATIVE])).unwrap();
        assert_eq!(got, PathBuf::from(NATIVE));
    }

    #[test]
    fn flatpak_used_when_only_it_exists() {
        let got = resolve(
            None,
            &env_of(&[("HOME", "/home/u")]),
            &exists_in(&[FLATPAK]),
        )
        .unwrap();
        assert_eq!(got, PathBuf::from(FLATPAK));
    }

    #[test]
    fn both_existing_is_ambiguous() {
        let err = resolve(
            None,
            &env_of(&[("HOME", "/home/u")]),
            &exists_in(&[NATIVE, FLATPAK]),
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Ambiguous { .. }));
    }

    #[test]
    fn neither_existing_lists_what_was_tried() {
        let err = resolve(None, &env_of(&[("HOME", "/home/u")]), &exists_in(&[])).unwrap_err();
        match err {
            ConfigError::NotFound { tried } => assert_eq!(tried.len(), 2),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn no_home_and_no_xdg_is_an_error() {
        let err = resolve(None, &env_of(&[]), &exists_in(&[])).unwrap_err();
        assert!(matches!(err, ConfigError::NoHome));
    }

    #[test]
    fn store_dir_per_platform() {
        let env = env_of(&[
            ("HOME", "/home/u"),
            ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
        ]);
        assert_eq!(
            store_dir(Os::Linux, &env).unwrap(),
            PathBuf::from("/home/u/.local/share/owl")
        );
        assert_eq!(
            store_dir(Os::Mac, &env).unwrap(),
            PathBuf::from("/home/u/Library/Application Support/owl")
        );
        assert_eq!(
            store_dir(Os::Windows, &env).unwrap(),
            PathBuf::from("C:\\Users\\u\\AppData\\Roaming").join("owl")
        );
    }

    #[test]
    fn store_dir_honours_absolute_xdg_data_home_only() {
        let abs = env_of(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data")]);
        assert_eq!(
            store_dir(Os::Linux, &abs).unwrap(),
            PathBuf::from("/data/owl")
        );
        let rel = env_of(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "data")]);
        assert_eq!(
            store_dir(Os::Linux, &rel).unwrap(),
            PathBuf::from("/home/u/.local/share/owl")
        );
    }

    #[test]
    fn store_dir_needs_a_home() {
        assert!(matches!(
            store_dir(Os::Linux, &env_of(&[])),
            Err(ConfigError::NoStoreHome)
        ));
        assert!(matches!(
            store_dir(Os::Windows, &env_of(&[("HOME", "/h")])),
            Err(ConfigError::NoStoreHome)
        ));
    }
}
