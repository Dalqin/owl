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
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|h| h.join(".local/share"))),
    };
    base.map(|b| b.join("owl")).ok_or(ConfigError::NoStoreHome)
}

/// Order: explicit `--cfg`, then the native location (`$XDG_CONFIG_HOME` or
/// `~/.config`), then the Flatpak location. If both native and Flatpak exist
/// it refuses to guess.
pub fn resolve(
    explicit: Option<PathBuf>,
    env: &dyn Fn(&str) -> Option<OsString>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, ConfigError> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let home = env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    // Inside a Flatpak sandbox `XDG_CONFIG_HOME` points at the app's own private
    // folder, not where OpenMW keeps its config, so only `~/.config` is meaningful.
    let sandboxed = env("FLATPAK_ID").is_some();
    let xdg = env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && !sandboxed);

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
