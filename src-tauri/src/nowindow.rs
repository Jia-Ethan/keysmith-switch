//! Suppress the console window Windows creates for console-subsystem children.
//!
//! The app is built with `windows_subsystem = "windows"` and so has no console of
//! its own; without `CREATE_NO_WINDOW` every spawned `curl`, `npm`, `taskkill` etc.
//! flashes a black window. On other platforms `no_window` is a no-op.

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for std::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

impl NoWindow for tokio::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

#[cfg(test)]
mod tests {
    /// Every process spawn in the crate must go through `no_window()`, otherwise Windows
    /// users get a flashing console. Files that legitimately need an exception go in `ALLOW`.
    #[test]
    fn every_command_new_is_marked_no_window() {
        const ALLOW: &[&str] = &["nowindow.rs"];
        let mut offenders = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    if ALLOW.contains(&name.as_str()) {
                        continue;
                    }
                    let text = std::fs::read_to_string(&path).unwrap();
                    // Only the non-test part of the file is checked.
                    let body = text.split("#[cfg(test)]").next().unwrap_or(&text);
                    let spawns = body.matches("Command::new(").count();
                    let marked = body.matches("no_window()").count()
                        + body.matches("configure_process_tree(").count();
                    if spawns > 0 && marked == 0 {
                        offenders.push(name);
                    }
                }
            }
        }
        assert!(offenders.is_empty(), "missing no_window(): {offenders:?}");
    }
}
