use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub dark_mode: bool,
    /// If true, do not show the "new version available" prompt
    pub hide_update_prompt: bool,
}

impl Settings {
    /// Get the path to the settings file
    pub fn config_path() -> Option<PathBuf> {
        // On macOS/Linux: ~/.xlrc
        // On Windows: %USERPROFILE%\.xlrc
        dirs::home_dir().map(|home| home.join(".xlrc"))
    }

    /// Load settings from the config file, creating it with defaults if it doesn't exist
    pub fn load() -> Self {
        let mut settings = Settings::default();

        let Some(path) = Self::config_path() else {
            return settings;
        };

        if !path.exists() {
            // Create the file with default settings
            let _ = settings.save();
            return settings;
        }

        // Read and parse the config file
        if let Ok(file) = fs::File::open(&path) {
            let reader = io::BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();

                    match key {
                        "dark_mode" => {
                            settings.dark_mode = value == "true" || value == "1";
                        }
                        "hide_update_prompt" => {
                            settings.hide_update_prompt = value == "true" || value == "1";
                        }
                        _ => {} // Ignore unknown keys
                    }
                }
            }
        }

        settings
    }

    /// Save settings to the config file
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not determine home directory",
            ));
        };

        let mut file = fs::File::create(&path)?;
        writeln!(file, "# xl spreadsheet settings")?;
        writeln!(file, "dark_mode={}", self.dark_mode)?;
        writeln!(file, "hide_update_prompt={}", self.hide_update_prompt)?;

        Ok(())
    }

    /// Update and save a single setting
    pub fn set_dark_mode(&mut self, dark_mode: bool) {
        self.dark_mode = dark_mode;
        let _ = self.save(); // Ignore errors on save
    }

    /// Update and save the "don't show update prompt again" setting
    pub fn set_hide_update_prompt(&mut self, hide: bool) {
        self.hide_update_prompt = hide;
        let _ = self.save(); // Ignore errors on save
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert!(!settings.dark_mode);
        assert!(!settings.hide_update_prompt);
    }

    #[test]
    fn test_config_path() {
        let path = Settings::config_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().ends_with(".xlrc"));
    }
}
