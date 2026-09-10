//! Native destination selection; never interprets an attachment name as a path.
use std::{path::PathBuf, sync::Arc};

pub fn attachment_source(
    parent: Arc<winit::window::Window>,
) -> impl std::future::Future<Output = Option<PathBuf>> + Send + 'static {
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(parent.as_ref())
        .set_title("Choose attachment")
        .pick_file();
    async move {
        let file = dialog.await?;
        drop(parent);
        Some(file.path().to_owned())
    }
}

pub fn attachment_destination(
    parent: Arc<winit::window::Window>,
    filename: &str,
) -> impl std::future::Future<Output = Option<PathBuf>> + Send + 'static {
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(parent.as_ref())
        .set_title("Save attachment")
        .set_file_name(safe_filename(filename))
        .save_file();
    async move {
        let file = dialog.await?;
        drop(parent);
        Some(file.path().to_owned())
    }
}

fn safe_filename(filename: &str) -> String {
    let name: String = filename
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        .take(120)
        .collect();
    let name = name.trim_matches(['.', ' ']);
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if name.is_empty() {
        "attachment".into()
    } else if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix.as_bytes(), [b'0'..=b'9']) || matches!(suffix, "¹" | "²" | "³")
            })
    {
        format!("_{name}")
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn suggested_names_are_single_safe_components() {
        for name in [
            "../../",
            "C:\\private\\file.png",
            "CON.png",
            "../NUL",
            "\0\n",
            "...",
        ] {
            let safe = super::safe_filename(name);
            assert!(!safe.is_empty() && !safe.contains(['/', '\\', ':', '\0', '\n']));
            assert!(!matches!(safe.as_str(), "." | ".." | "CON.png" | "NUL"));
        }
        assert_eq!(super::safe_filename("photo.png"), "photo.png");
        assert_eq!(super::safe_filename("report.pdf"), "report.pdf");
        assert_eq!(super::safe_filename("archive.bin"), "archive.bin");
        assert_eq!(super::safe_filename("..."), "attachment");
        for prefix in ["COM", "LPT", "com", "lpt"] {
            for digit in ["1", "9", "¹", "²", "³"] {
                let name = format!("{prefix}{digit}.png");
                assert_eq!(super::safe_filename(&name), format!("_{name}"));
            }
        }
        for name in ["COM10.png", "LPT12.png", "COM¹photo.png", "COM⁴.png"] {
            assert_eq!(super::safe_filename(name), name);
        }
    }
}
