use std::{ffi::OsString, path::PathBuf};

const NOTSLACK_DATA_DIR_ENV: &str = "NOTSLACK_DATA_DIR";

pub fn app_data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(NOTSLACK_DATA_DIR_ENV) {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(format!(
                "{NOTSLACK_DATA_DIR_ENV} must be an absolute path"
            ));
        }
        return Ok(path);
    }
    let home = home_dir()?;
    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Application Support/notslack"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(home.join(".config/notslack"))
    }
}

fn home_dir() -> Result<PathBuf, String> {
    home_dir_from_env(
        std::env::var_os("HOME"),
        std::env::var_os("USERPROFILE"),
        cfg!(target_os = "windows"),
    )
}

fn home_dir_from_env(
    home: Option<OsString>,
    userprofile: Option<OsString>,
    use_windows_profile: bool,
) -> Result<PathBuf, String> {
    if let Some(home) = home {
        return Ok(PathBuf::from(home));
    }
    if use_windows_profile {
        if let Some(userprofile) = userprofile {
            return Ok(PathBuf::from(userprofile));
        }
        return Err("missing HOME or USERPROFILE: environment variable not found".to_string());
    }
    Err("missing HOME: environment variable not found".to_string())
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::home_dir_from_env;

    #[test]
    fn home_dir_prefers_home() {
        let home = home_dir_from_env(
            Some(OsString::from("C:\\Users\\home")),
            Some(OsString::from("C:\\Users\\profile")),
            true,
        )
        .expect("home dir");

        assert_eq!(home, PathBuf::from("C:\\Users\\home"));
    }

    #[test]
    fn home_dir_uses_windows_userprofile_when_home_is_missing() {
        let home = home_dir_from_env(None, Some(OsString::from("C:\\Users\\alex")), true)
            .expect("home dir");

        assert_eq!(home, PathBuf::from("C:\\Users\\alex"));
    }

    #[test]
    fn home_dir_rejects_missing_home_without_windows_profile_fallback() {
        let error = home_dir_from_env(None, Some(OsString::from("C:\\Users\\alex")), false)
            .expect_err("missing home");

        assert_eq!(error, "missing HOME: environment variable not found");
    }
}
