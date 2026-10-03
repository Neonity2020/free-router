use std::{
    env,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) struct Paths {
    pub(crate) root: PathBuf,
    pub(crate) settings_file: PathBuf,
    pub(crate) data_root: PathBuf,
    pub(crate) gateway_key_file: PathBuf,
}

fn user_data_root() -> PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/Free Router");
    }
    #[cfg(target_os = "windows")]
    if let Some(app_data) = env::var_os("APPDATA") {
        return PathBuf::from(app_data).join("Free Router");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(config) = env::var_os("XDG_CONFIG_HOME").filter(|p| Path::new(p).is_absolute())
        {
            return PathBuf::from(config).join("free-router");
        }
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(".config/free-router");
        }
    }
    panic!("Cannot resolve user configuration directory; set SETTINGS_FILE explicitly");
}

fn resource_root(executable_root: &Path, manifest: &Path) -> (PathBuf, bool) {
    let installed = executable_root.join("free-router-resources");
    if installed.join("frontend/dist/index.html").is_file() {
        return (installed, false);
    }
    if executable_root.join("frontend/dist/index.html").is_file() {
        return (executable_root.to_path_buf(), true);
    }
    // Only an actual Cargo checkout binary may use the source tree default.
    if executable_root.starts_with(manifest.join("target")) {
        return (manifest.parent().unwrap_or(manifest).to_path_buf(), true);
    }
    (executable_root.to_path_buf(), false)
}

pub(crate) fn resolve() -> Paths {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let executable_root = env::current_exe()
        .expect("Cannot resolve executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    let (default_root, local_data) = resource_root(&executable_root, &manifest);
    let override_root = env::var_os("FREE_ROUTER_RESOURCE_ROOT").map(PathBuf::from);
    let root = override_root.clone().unwrap_or(default_root);
    let settings_file = env::var_os("SETTINGS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let data_root = if local_data || override_root.is_some() {
                root.clone()
            } else {
                user_data_root()
            };
            data_root.join("settings.local.json")
        });
    // Resolve relative overrides once, so locking and displayed paths agree.
    let settings_file = if settings_file.is_absolute() {
        settings_file
    } else {
        env::current_dir()
            .expect("Cannot resolve current directory")
            .join(settings_file)
    };
    let data_root = settings_file
        .parent()
        .expect("Settings file has no parent")
        .to_path_buf();
    Paths {
        gateway_key_file: data_root.join("gateway-key.local.txt"),
        settings_file,
        data_root,
        root,
    }
}

impl Paths {
    fn lock_file(&self) -> Result<File, String> {
        std::fs::create_dir_all(&self.data_root)
            .map_err(|error| format!("无法创建配置目录：{error}"))?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(self.data_root.join(".free-router.lock"))
            .map_err(|error| format!("无法打开配置锁：{error}"))
    }

    /// The gateway holds this guard until shutdown, excluding offline CLI writers.
    pub(crate) fn lock_for_server(&self) -> Result<File, String> {
        let file = self.lock_file()?;
        file.try_lock()
            .map_err(|error| format!("配置正在使用中，不能启动另一个网关：{error}"))?;
        Ok(file)
    }

    /// Serialize offline CLI read-modify-write transactions, with a bounded wait.
    pub(crate) async fn lock_for_cli(&self) -> Result<File, String> {
        let file = self.lock_file()?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(std::fs::TryLockError::WouldBlock)
                    if tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err("配置正在使用中；请连接正在运行的网关并提供正确的管理密钥，或停止网关后重试".into());
                }
                Err(error) => return Err(format!("无法锁定配置：{error}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_binary_does_not_use_compile_time_source_tree() {
        let manifest = Path::new("/source/backend");
        let installed = Path::new("/installed/bin");
        assert_eq!(
            resource_root(installed, manifest),
            (installed.to_path_buf(), false)
        );
        assert_eq!(
            resource_root(Path::new("/source/backend/target/debug"), manifest),
            (PathBuf::from("/source"), true)
        );
    }
}
