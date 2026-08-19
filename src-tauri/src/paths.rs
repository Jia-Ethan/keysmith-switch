use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::ToolKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub home: PathBuf,
    pub db: PathBuf,
    pub prompts: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
    pub lock: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        if let Ok(override_home) = std::env::var("KEYSMITH_SWITCH_HOME") {
            let trimmed = override_home.trim();
            if !trimmed.is_empty() {
                return Ok(Self::from_home(trimmed));
            }
        }
        let home = dirs::home_dir()
            .ok_or_else(|| Error::message("cannot resolve user home directory"))?
            .join(".keysmith-switch");
        Ok(Self::from_home(home))
    }

    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            db: home.join("keysmith-switch.db"),
            prompts: home.join("prompts"),
            backups: home.join("backups"),
            logs: home.join("logs"),
            lock: home.join(".lock"),
            home,
        }
    }

    pub fn in_temp_dir() -> Result<(tempfile::TempDir, Self)> {
        let dir = tempfile::tempdir()?;
        let paths = Self::from_home(dir.path().join(".keysmith-switch"));
        paths.ensure()?;
        Ok((dir, paths))
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.home)?;
        fs::create_dir_all(&self.prompts)?;
        fs::create_dir_all(&self.backups)?;
        fs::create_dir_all(&self.logs)?;
        for tool in ToolKind::ALL {
            fs::create_dir_all(self.tool_dir(tool))?;
        }
        Ok(())
    }

    pub fn tool_dir(&self, tool: ToolKind) -> PathBuf {
        self.prompts.join(tool.as_str())
    }

    pub fn prompt_file(&self, tool: ToolKind, id: &str) -> PathBuf {
        self.tool_dir(tool).join(format!("{id}.md"))
    }

    pub fn backup_dir(&self, operation_id: &str) -> PathBuf {
        self.backups.join(operation_id)
    }

    pub fn db_backup_path(&self, label: &str) -> PathBuf {
        self.backups.join(format!("keysmith-switch.db.{label}"))
    }
}

pub fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let tmp = path.with_file_name(format!(
        ".{file_name}.keysmith-switch-tmp-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let write_tmp = || -> Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        Ok(())
    };
    if let Err(error) = write_tmp() {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    match replace_file(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

fn replace_file(tmp: &Path, dest: &Path) -> Result<()> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(error) if dest.exists() => {
            #[cfg(windows)]
            {
                fs::remove_file(dest)?;
                fs::rename(tmp, dest)?;
                return Ok(());
            }
            #[cfg(not(windows))]
            {
                Err(error.into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

pub fn read_to_string_if_exists(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?))
}
