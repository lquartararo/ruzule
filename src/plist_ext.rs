use crate::error::Result;
use plist::Value;
use std::path::{Path, PathBuf};

pub struct PlistFile {
    pub path: PathBuf,
    pub data: plist::Dictionary,
    app_path: Option<PathBuf>,
}

impl PlistFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = plist::from_file::<_, plist::Dictionary>(&path)?;
        Ok(Self {
            path,
            data,
            app_path: None,
        })
    }

    pub fn open_with_app_path<P: AsRef<Path>>(path: P, app_path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = plist::from_file::<_, plist::Dictionary>(&path)?;
        Ok(Self {
            path,
            data,
            app_path: Some(app_path.as_ref().to_path_buf()),
        })
    }

    pub fn try_open<P: AsRef<Path>>(path: P) -> Option<Self> {
        Self::open(path).ok()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), Value::String(value.to_string()));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.data.insert(key.to_string(), Value::Boolean(value));
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    pub fn save(&self) -> Result<()> {
        plist::to_file_xml(&self.path, &self.data)?;
        Ok(())
    }

    pub fn remove_uisd(&mut self) -> bool {
        let removed = self.remove("UISupportedDevices");
        if removed {
            let _ = self.save();
            println!("[*] removed UISupportedDevices");
        }
        removed
    }

    pub fn enable_documents(&mut self) -> bool {
        let mut changed = false;

        if self.get_string("UISupportsDocumentBrowser") != Some("true") {
            self.set_bool("UISupportsDocumentBrowser", true);
            changed = true;
        }
        if self.get_string("UIFileSharingEnabled") != Some("true") {
            self.set_bool("UIFileSharingEnabled", true);
            changed = true;
        }

        if changed {
            let _ = self.save();
            println!("[*] enabled documents support");
        }
        changed
    }

    pub fn change_name(&mut self, name: &str) -> bool {
        let current_name = self.get_string("CFBundleName").map(|s| s.to_string());
        let current_display = self.get_string("CFBundleDisplayName").map(|s| s.to_string());

        if current_name.as_deref() == Some(name) && current_display.as_deref() == Some(name) {
            return false;
        }

        self.set_string("CFBundleName", name);
        self.set_string("CFBundleDisplayName", name);
        let _ = self.save();
        println!("[*] changed name to \"{}\"", name);

        // Update localized names
        if let Some(ref app_path) = self.app_path {
            let mut changed_count = 0;
            if let Ok(entries) = glob::glob(&format!("{}/*.lproj", app_path.display())) {
                for entry in entries.flatten() {
                    let strings_path = entry.join("InfoPlist.strings");
                    if let Ok(mut pl) = PlistFile::open(&strings_path) {
                        pl.set_string("CFBundleName", name);
                        pl.set_string("CFBundleDisplayName", name);
                        if pl.save().is_ok() {
                            changed_count += 1;
                        }
                    }
                }
            }
            if changed_count > 0 {
                println!("[*] changed \x1b[96m{}\x1b[0m localized names", changed_count);
            }
        }
        true
    }

    pub fn change_version(&mut self, version: &str) -> bool {
        let current_ver = self.get_string("CFBundleVersion").map(|s| s.to_string());
        let current_short = self.get_string("CFBundleShortVersionString").map(|s| s.to_string());

        if current_ver.as_deref() == Some(version) && current_short.as_deref() == Some(version) {
            return false;
        }

        self.set_string("CFBundleVersion", version);
        self.set_string("CFBundleShortVersionString", version);
        let _ = self.save();
        println!("[*] changed version to \"{}\"", version);
        true
    }

    pub fn change_bundle_id(&mut self, bundle_id: &str) -> bool {
        let orig = match self.get_string("CFBundleIdentifier") {
            Some(id) => id.to_string(),
            None => return false,
        };

        if orig == bundle_id {
            return false;
        }

        self.set_string("CFBundleIdentifier", bundle_id);
        let _ = self.save();
        println!("[*] changed bundle id to \"{}\"", bundle_id);

        // Update extension bundle IDs
        if let Some(ref app_path) = self.app_path {
            let mut changed_count = 0;
            let pattern = format!("{}/*/*.appex", app_path.display());
            if let Ok(entries) = glob::glob(&pattern) {
                for entry in entries.flatten() {
                    let plist_path = entry.join("Info.plist");
                    if let Ok(mut pl) = PlistFile::open(&plist_path) {
                        if let Some(current) = pl.get_string("CFBundleIdentifier").map(|s| s.to_string()) {
                            let new_id = current.replace(&orig, bundle_id);
                            pl.set_string("CFBundleIdentifier", &new_id);
                            if pl.save().is_ok() {
                                changed_count += 1;
                            }
                        }
                    }
                }
            }
            if changed_count > 0 {
                println!("[*] changed \x1b[96m{}\x1b[0m other bundle ids", changed_count);
            }
        }
        true
    }

    pub fn change_minimum_version(&mut self, minimum: &str) -> bool {
        let current = self.get_string("MinimumOSVersion").map(|s| s.to_string());

        if current.as_deref() == Some(minimum) {
            return false;
        }

        self.set_string("MinimumOSVersion", minimum);
        let _ = self.save();
        println!("[*] changed minimum version to \"{}\"", minimum);
        true
    }

    pub fn merge_plist<P: AsRef<Path>>(&mut self, path: P) -> Result<bool> {
        let other = PlistFile::open(path)?;
        let mut changed = false;

        let keys: Vec<String> = other.data.keys().cloned().collect();
        for key in &keys {
            if let Some(value) = other.data.get(key) {
                self.data.insert(key.clone(), value.clone());
                changed = true;
            }
        }

        if changed {
            self.save()?;
            println!("[*] merged plist ({} keys)", keys.len());
        }

        Ok(changed)
    }
}
