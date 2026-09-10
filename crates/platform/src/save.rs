//! Native destination selection; never interprets an attachment name as a path.
use std::{path::PathBuf, sync::Arc};

pub fn attachment_destination(
    parent: Arc<winit::window::Window>,
    filename: &str,
) -> impl std::future::Future<Output = Option<PathBuf>> + Send + 'static {
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(parent.as_ref())
        .set_title("Save original image")
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
        "attachment.png".into()
    } else if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
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
    }
}
