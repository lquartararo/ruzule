use anyhow::{Context, Result};
use plist::{Dictionary, Value};
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct PlistWrapper {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub bundle_path: PathBuf,
    data: Dictionary,
    modified: bool,
}

impl PlistWrapper {
    pub fn new<P: AsRef<Path>>(path: P, bundle_path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let bundle_path = bundle_path.as_ref().to_path_buf();
        
        let file = File::open(&path)
            .with_context(|| format!("Failed to open plist: {}", path.display()))?;
        let data: Dictionary = plist::from_reader(file)
            .with_context(|| format!("Failed to parse plist: {}", path.display()))?;
        
        Ok(Self {
            path,
            bundle_path,
            data,
            modified: false,
        })
    }

    #[allow(dead_code)]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_string())
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
        self.modified = true;
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.modified = true;
        self.data.remove(key)
    }

    pub fn save(&self) -> Result<()> {
        let file = File::create(&self.path)
            .with_context(|| format!("Failed to create plist: {}", self.path.display()))?;
        plist::to_writer_xml(file, &self.data)
            .with_context(|| format!("Failed to write plist: {}", self.path.display()))?;
        Ok(())
    }

    pub fn change_name(&mut self, name: &str) {
        self.set("CFBundleDisplayName", Value::String(name.to_string()));
        self.set("CFBundleName", Value::String(name.to_string()));
        println!("[*] changed app name to {}", name);
    }

    pub fn change_version(&mut self, version: &str) {
        self.set("CFBundleShortVersionString", Value::String(version.to_string()));
        self.set("CFBundleVersion", Value::String(version.to_string()));
        println!("[*] changed app version to {}", version);
    }

    pub fn change_bundle_id(&mut self, bundle_id: &str) {
        self.set("CFBundleIdentifier", Value::String(bundle_id.to_string()));
        println!("[*] changed bundle id to {}", bundle_id);
    }

    pub fn change_minimum_version(&mut self, version: &str) {
        self.set("MinimumOSVersion", Value::String(version.to_string()));
        println!("[*] changed minimum OS version to {}", version);
    }

    pub fn remove_uisd(&mut self) {
        if self.remove("UISupportedDevices").is_some() {
            println!("[*] removed UISupportedDevices");
        } else {
            println!("[?] UISupportedDevices not present");
        }
    }

    pub fn enable_documents(&mut self) {
        self.set("UISupportsDocumentBrowser", Value::Boolean(true));
        self.set("UIFileSharingEnabled", Value::Boolean(true));
        println!("[*] enabled documents support");
    }

    pub fn merge_plist<P: AsRef<Path>>(&mut self, other_path: P) -> Result<()> {
        let file = File::open(other_path.as_ref())?;
        let other: Dictionary = plist::from_reader(file)?;
        
        for (key, value) in other {
            self.data.insert(key, value);
        }
        self.modified = true;
        println!("[*] merged plist");
        Ok(())
    }

    pub fn executable_name(&self) -> Option<&str> {
        self.get_string("CFBundleExecutable")
    }
}

impl Drop for PlistWrapper {
    fn drop(&mut self) {
        if self.modified {
            let _ = self.save();
        }
    }
}
