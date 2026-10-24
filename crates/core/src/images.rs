//! Reading the pictures a mod's author put in the installer (the banner and the
//! per-option screenshots) so a front end can show them.
//!
//! The path comes from a file inside the mod, so it is never trusted: it must
//! stay inside the mod folder, symbolic links are refused, and the file must
//! really be an image of a kind a browser shows.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Largest picture that is read.
const MAX_IMAGE_LEN: u64 = 4 << 20;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImageError {
    #[error("the picture's path is not inside the mod's folder: {0}")]
    Unsafe(String),
    #[error("no such picture in the mod's folder: {0}")]
    NotFound(String),
    #[error("the picture is larger than {} MB", MAX_IMAGE_LEN >> 20)]
    TooLarge,
    #[error("the file is not a picture of a kind that can be shown")]
    NotAnImage,
    #[error("{0}")]
    Io(String),
}

/// The picture at `relative` inside `root`, as a `data:` address. The path may
/// use backslashes and any letter case, as installer files written on Windows do.
pub fn read_image(root: &Path, relative: &Path) -> Result<String, ImageError> {
    let path = resolve(root, relative)?;
    let len = fs::metadata(&path)
        .map_err(|e| ImageError::Io(e.to_string()))?
        .len();
    if len > MAX_IMAGE_LEN {
        return Err(ImageError::TooLarge);
    }
    let bytes = fs::read(&path).map_err(|e| ImageError::Io(e.to_string()))?;
    let mime = sniff(&bytes).ok_or(ImageError::NotAnImage)?;
    Ok(format!("data:{mime};base64,{}", base64(&bytes)))
}

/// Finds the file, matching each part of the path without regard to letter
/// case, and refusing anything that could lead out of `root`.
fn resolve(root: &Path, relative: &Path) -> Result<PathBuf, ImageError> {
    let shown = relative.to_string_lossy().into_owned();
    let unsafe_path = || ImageError::Unsafe(shown.clone());
    // Installer files use backslashes whatever the system.
    let normalised = shown.replace('\\', "/");
    let mut found = root.to_path_buf();
    let mut parts = 0;
    for part in Path::new(&normalised).components() {
        let name = match part {
            Component::Normal(n) => n.to_string_lossy().into_owned(),
            Component::CurDir => continue,
            _ => return Err(unsafe_path()),
        };
        if name.contains('\0') {
            return Err(unsafe_path());
        }
        found = child_ci(&found, &name).ok_or_else(|| ImageError::NotFound(shown.clone()))?;
        // A link could point anywhere; real files only.
        let meta = fs::symlink_metadata(&found).map_err(|e| ImageError::Io(e.to_string()))?;
        if meta.file_type().is_symlink() {
            return Err(unsafe_path());
        }
        parts += 1;
    }
    if parts == 0 {
        return Err(unsafe_path());
    }
    if !fs::symlink_metadata(&found).is_ok_and(|m| m.is_file()) {
        return Err(ImageError::NotFound(shown));
    }
    Ok(found)
}

/// `dir/name`, found regardless of letter case (an exact match wins).
fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if fs::symlink_metadata(&exact).is_ok() {
        return Some(exact);
    }
    let want = name.to_lowercase();
    let mut matches: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().to_lowercase() == want)
        .map(|e| e.path())
        .collect();
    matches.sort();
    matches.into_iter().next()
}

/// The kind of picture, from what the file starts with (not from its name).
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The smallest valid PNG (1x1), so the test does not depend on a real picture.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\0\0\0\rIDATx\x9cc\xf8\xff\xff?\0\x05\xfe\x02\xfe\xdc\xccY\xe7\0\0\0\0IEND\xaeB`\x82";

    fn mod_with_picture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Screenshots")).unwrap();
        fs::write(dir.path().join("Screenshots").join("Header.PNG"), PNG).unwrap();
        dir
    }

    #[test]
    fn base64_matches_the_standard_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn reads_a_picture_whatever_the_slashes_and_letter_case() {
        let dir = mod_with_picture();
        for written in [
            "Screenshots/Header.PNG",
            "Screenshots\\header.png",
            "screenshots\\HEADER.png",
            "./Screenshots/header.png",
        ] {
            let uri = read_image(dir.path(), Path::new(written))
                .unwrap_or_else(|e| panic!("{written}: {e}"));
            assert!(
                uri.starts_with("data:image/png;base64,iVBORw0KGgo"),
                "{uri}"
            );
        }
    }

    #[test]
    fn the_kind_comes_from_the_contents_not_the_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.png"), b"not a picture at all").unwrap();
        fs::write(dir.path().join("b.jpg"), PNG).unwrap();
        assert_eq!(
            read_image(dir.path(), Path::new("a.png")),
            Err(ImageError::NotAnImage)
        );
        assert!(read_image(dir.path(), Path::new("b.jpg"))
            .unwrap()
            .starts_with("data:image/png"));
    }

    #[test]
    fn refuses_paths_that_leave_the_folder() {
        let dir = mod_with_picture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.png"), PNG).unwrap();
        for bad in [
            "../secret.png",
            "Screenshots/../../secret.png",
            "..\\secret.png",
            outside.path().join("secret.png").to_str().unwrap(),
            "",
            ".",
        ] {
            let got = read_image(dir.path(), Path::new(bad));
            assert!(
                matches!(got, Err(ImageError::Unsafe(_))),
                "{bad:?} gave {got:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_link_that_leads_elsewhere() {
        let dir = mod_with_picture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.png"), PNG).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.png"),
            dir.path().join("link.png"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked-dir")).unwrap();
        assert!(matches!(
            read_image(dir.path(), Path::new("link.png")),
            Err(ImageError::Unsafe(_))
        ));
        assert!(matches!(
            read_image(dir.path(), Path::new("linked-dir/secret.png")),
            Err(ImageError::Unsafe(_))
        ));
    }

    #[test]
    fn says_so_when_the_picture_is_missing_or_too_big() {
        let dir = mod_with_picture();
        assert!(matches!(
            read_image(dir.path(), Path::new("nope.png")),
            Err(ImageError::NotFound(_))
        ));
        assert!(matches!(
            read_image(dir.path(), Path::new("Screenshots")),
            Err(ImageError::NotFound(_))
        ));
        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_LEN as usize + 1, 0);
        fs::write(dir.path().join("big.png"), big).unwrap();
        assert_eq!(
            read_image(dir.path(), Path::new("big.png")),
            Err(ImageError::TooLarge)
        );
    }
}
