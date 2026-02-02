use crate::deb;
use crate::error::{Result, RuzuleError};
use crate::executable::{Executable, MainExecutable};
use crate::plist_ext::PlistFile;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct AppBundle {
    pub path: PathBuf,
    pub plist: PlistFile,
    pub executable: MainExecutable,
    cached_executables: Option<Vec<PathBuf>>,
}

impl AppBundle {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let plist_path = path.join("Info.plist");

        let plist = PlistFile::open_with_app_path(&plist_path, &path)?;

        let exec_name = plist
            .get_string("CFBundleExecutable")
            .ok_or_else(|| RuzuleError::InvalidAppBundle("No CFBundleExecutable".to_string()))?;

        let exec_path = path.join(exec_name);
        let executable = MainExecutable::new(&exec_path, &path)?;

        Ok(Self {
            path,
            plist,
            executable,
            cached_executables: None,
        })
    }

    pub fn remove<P: AsRef<Path>>(&self, names: &[P]) -> bool {
        let mut existed = false;

        for name in names {
            let name = name.as_ref();
            let path = if name.starts_with(&self.path) {
                name.to_path_buf()
            } else {
                self.path.join(name)
            };

            if !path.exists() {
                continue;
            }

            let result = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };

            if result.is_ok() {
                existed = true;
            }
        }

        existed
    }

    pub fn remove_watch_apps(&mut self) {
        let names = ["Watch", "WatchKit", "com.apple.WatchPlaceholder"];
        if self.remove(&names.map(Path::new)) {
            println!("[*] removed watch app");
        }
    }

    fn get_executables(&self) -> Vec<PathBuf> {
        let mut executables = Vec::new();
        let patterns = [
            format!("{}/**/*.dylib", self.path.display()),
            format!("{}/**/*.appex", self.path.display()),
            format!("{}/**/*.framework", self.path.display()),
        ];

        for pattern in patterns {
            if let Ok(paths) = glob::glob(&pattern) {
                for path in paths.flatten() {
                    executables.push(path);
                }
            }
        }

        executables
    }

    pub fn fakesign_all(&mut self) -> Result<()> {
        if self.cached_executables.is_none() {
            self.cached_executables = Some(self.get_executables());
        }

        let mut count = 0;

        if self.executable.fakesign()? {
            count += 1;
        }

        if let Some(ref executables) = self.cached_executables {
            for exec_path in executables {
                let result = if exec_path
                    .extension()
                    .map(|e| e == "dylib")
                    .unwrap_or(false)
                {
                    Executable::new(exec_path)?.fakesign()
                } else {
                    // It's a bundle, get its executable
                    let plist_path = exec_path.join("Info.plist");
                    if let Ok(pl) = PlistFile::open(&plist_path) {
                        if let Some(exec_name) = pl.get_string("CFBundleExecutable") {
                            Executable::new(exec_path.join(exec_name))?.fakesign()
                        } else {
                            Ok(false)
                        }
                    } else {
                        Ok(false)
                    }
                };

                if result.unwrap_or(false) {
                    count += 1;
                }
            }
        }

        println!("[*] fakesigned \x1b[96m{}\x1b[0m item(s)", count);
        Ok(())
    }

    pub fn thin_all(&mut self) -> Result<()> {
        if self.cached_executables.is_none() {
            self.cached_executables = Some(self.get_executables());
        }

        let mut count = 0;

        if self.executable.thin().unwrap_or(false) {
            count += 1;
        }

        if let Some(ref executables) = self.cached_executables {
            for exec_path in executables {
                let result = if exec_path
                    .extension()
                    .map(|e| e == "dylib")
                    .unwrap_or(false)
                {
                    Executable::new(exec_path)?.thin()
                } else {
                    let plist_path = exec_path.join("Info.plist");
                    if let Ok(pl) = PlistFile::open(&plist_path) {
                        if let Some(exec_name) = pl.get_string("CFBundleExecutable") {
                            Executable::new(exec_path.join(exec_name))?.thin()
                        } else {
                            Ok(false)
                        }
                    } else {
                        Ok(false)
                    }
                };

                if result.unwrap_or(false) {
                    count += 1;
                }
            }
        }

        println!("[*] thinned \x1b[96m{}\x1b[0m item(s)", count);
        Ok(())
    }

    pub fn remove_all_extensions(&mut self) {
        let names = ["Extensions", "PlugIns"];
        if self.remove(&names.map(Path::new)) {
            println!("[*] removed app extensions");
        }
    }

    pub fn remove_encrypted_extensions(&mut self) -> Result<()> {
        let mut removed = Vec::new();

        let pattern = format!("{}/*/*.appex", self.path.display());
        if let Ok(paths) = glob::glob(&pattern) {
            for plugin_path in paths.flatten() {
                if let Ok(bundle) = AppBundle::new(&plugin_path) {
                    if bundle.executable.is_encrypted().unwrap_or(false)
                        && self.remove(&[&plugin_path])
                    {
                        removed.push(bundle.executable.inner.name);
                    }
                }
            }
        }

        if !removed.is_empty() {
            println!("[*] removed encrypted plugins: {}", removed.join(", "));
        }

        Ok(())
    }

    pub fn change_icon<P: AsRef<Path>, Q: AsRef<Path>>(&mut self, icon_path: P, _tmpdir: Q) -> Result<()> {
        let icon_path = icon_path.as_ref();

        // Load and convert image to PNG
        let img = image::open(icon_path)?;

        let uid = format!("ruzule_{}a", &uuid::Uuid::new_v4().simple().to_string()[..7]);
        let i60 = format!("{}60x60", uid);
        let i76 = format!("{}76x76", uid);

        // Create resized icons
        let img_120 = img.resize_exact(120, 120, image::imageops::FilterType::Lanczos3);
        let img_152 = img.resize_exact(152, 152, image::imageops::FilterType::Lanczos3);

        img_120.save(self.path.join(format!("{}@2x.png", i60)))?;
        img_152.save(self.path.join(format!("{}@2x~ipad.png", i76)))?;

        // Update plist
        let primary_icon = plist::Value::Dictionary({
            let mut d = plist::Dictionary::new();
            d.insert(
                "CFBundleIconFiles".to_string(),
                plist::Value::Array(vec![plist::Value::String(i60.clone())]),
            );
            d.insert(
                "CFBundleIconName".to_string(),
                plist::Value::String(uid.clone()),
            );
            d
        });

        let primary_icon_ipad = plist::Value::Dictionary({
            let mut d = plist::Dictionary::new();
            d.insert(
                "CFBundleIconFiles".to_string(),
                plist::Value::Array(vec![
                    plist::Value::String(i60),
                    plist::Value::String(i76),
                ]),
            );
            d.insert(
                "CFBundleIconName".to_string(),
                plist::Value::String(uid),
            );
            d
        });

        // Get or create CFBundleIcons
        let mut icons = if let Some(plist::Value::Dictionary(d)) = self.plist.get("CFBundleIcons")
        {
            d.clone()
        } else {
            plist::Dictionary::new()
        };
        icons.insert("CFBundlePrimaryIcon".to_string(), primary_icon);
        self.plist
            .set("CFBundleIcons", plist::Value::Dictionary(icons));

        // Get or create CFBundleIcons~ipad
        let mut icons_ipad =
            if let Some(plist::Value::Dictionary(d)) = self.plist.get("CFBundleIcons~ipad") {
                d.clone()
            } else {
                plist::Dictionary::new()
            };
        icons_ipad.insert("CFBundlePrimaryIcon".to_string(), primary_icon_ipad);
        self.plist
            .set("CFBundleIcons~ipad", plist::Value::Dictionary(icons_ipad));

        self.plist.save()?;
        println!("[*] updated app icon");

        Ok(())
    }

    pub fn inject(&mut self, tweaks: &mut HashMap<String, PathBuf>, tmpdir: &Path, use_frameworks_dir: bool) -> Result<()> {
        let ent_path = self.path.join("ruzule.entitlements");
        let plugins_dir = self.path.join("PlugIns");
        let frameworks_dir = self.path.join("Frameworks");

        let has_entitlements = self.executable.write_entitlements(&ent_path)?;

        // Remove signature before injecting
        self.executable.inner.remove_signature()?;

        // Create directories if needed
        let has_appex = tweaks.keys().any(|k| k.ends_with(".appex"));
        let has_injectable = tweaks
            .keys()
            .any(|k| k.ends_with(".deb") || k.ends_with(".dylib") || k.ends_with(".framework"));

        if has_appex {
            fs::create_dir_all(&plugins_dir)?;
        }

        if has_injectable && use_frameworks_dir {
            fs::create_dir_all(&frameworks_dir)?;
            self.executable
                .add_rpath("@executable_path/Frameworks")?;
        }

        // Extract .deb files first (modifies tweaks)
        let deb_keys: Vec<String> = tweaks
            .keys()
            .filter(|k| k.ends_with(".deb"))
            .cloned()
            .collect();

        for deb_name in deb_keys {
            if let Some(deb_path) = tweaks.get(&deb_name).cloned() {
                deb::extract_deb(&deb_path, tweaks, tmpdir)?;
            }
        }

        let mut needed: HashSet<String> = HashSet::new();

        // Process each tweak
        for (bn, path) in tweaks.iter() {
            // Skip symlinks
            if path.is_symlink() {
                continue;
            }

            if bn.ends_with(".appex") {
                let fpath = plugins_dir.join(bn);
                delete_if_exists(&fpath, bn);
                copy_dir_all(path, &fpath)?;
                println!("[*] injected {}", bn);
            } else if bn.ends_with(".dylib") {
                // Copy to temp, fix deps, then move to destination
                let temp_path = tmpdir.join(bn);
                fs::copy(path, &temp_path)?;

                let exec = Executable::new(&temp_path)?;
                exec.fix_common_dependencies(&mut needed)?;
                exec.fix_dependencies(tweaks)?;
                if use_frameworks_dir {
                    exec.fix_install_name(tweaks)?;
                }

                let (fpath, inject_path) = if use_frameworks_dir {
                    (frameworks_dir.join(bn), format!("@rpath/{}", bn))
                } else {
                    (self.path.join(bn), format!("@executable_path/{}", bn))
                };
                delete_if_exists(&fpath, bn);

                self.executable.inject_dylib(&inject_path)?;
                fs::rename(&temp_path, &fpath)?;
                println!("[*] injected {}", bn);
            } else if bn.ends_with(".framework") {
                let framework_name = bn.strip_suffix(".framework").unwrap();
                let (fpath, inject_path) = if use_frameworks_dir {
                    (frameworks_dir.join(bn), format!("@rpath/{}/{}", bn, framework_name))
                } else {
                    (self.path.join(bn), format!("@executable_path/{}/{}", bn, framework_name))
                };
                delete_if_exists(&fpath, bn);

                self.executable.inject_dylib(&inject_path)?;
                copy_dir_all(path, &fpath)?;
                println!("[*] injected {}", bn);
            } else if bn.ends_with(".bundle") {
                let fpath = self.path.join(bn);
                delete_if_exists(&fpath, bn);
                copy_dir_all(path, &fpath)?;
                println!("[*] injected {}", bn);
            } else {
                // Unknown file type, copy to app root
                let fpath = self.path.join(bn);
                delete_if_exists(&fpath, bn);
                if path.is_dir() {
                    copy_dir_all(path, &fpath)?;
                } else {
                    fs::copy(path, &fpath)?;
                }
                println!("[*] injected {}", bn);
            }
        }

        // Orion has a weak dependency to substrate
        if needed.contains("orion.") {
            needed.insert("substrate.".to_string());
        }

        // Auto-inject needed common dependencies (ElleKit, etc.)
        for missing in &needed {
            if let Some(framework) = crate::frameworks::get_framework_for_dep(missing) {
                let framework_name = framework.framework_name();
                let dest_dir = if use_frameworks_dir { &frameworks_dir } else { &self.path };
                let fpath = dest_dir.join(&framework_name);

                if !delete_if_exists(&fpath, &framework_name) {
                    println!("[*] auto-injected {}", framework_name);
                }

                framework.extract_to(dest_dir)?;
            }
        }

        // Restore entitlements
        if has_entitlements {
            self.executable.sign_with_entitlements(&ent_path)?;
            println!("[*] restored entitlements");
            fs::remove_file(&ent_path)?;
        }

        Ok(())
    }

    /// Patch the main executable and all plugins to fix share sheet, widgets, VPNs, etc.
    /// Injects zxPluginsInject.dylib into all executables.
    pub fn patch_plugins(&mut self) -> Result<()> {
        use crate::frameworks::ZX_PLUGINS_INJECT;
        use crate::macho;
        use crate::sign;

        // Ensure Frameworks directory exists
        let frameworks_dir = self.path.join("Frameworks");
        fs::create_dir_all(&frameworks_dir)?;

        // Write zxPluginsInject.dylib
        let dylib_dest = frameworks_dir.join("zxPluginsInject.dylib");
        fs::write(&dylib_dest, ZX_PLUGINS_INJECT)?;

        // Add rpath if needed
        self.executable.add_rpath("@executable_path/Frameworks")?;

        // Inject into main executable
        let inject_path = "@rpath/zxPluginsInject.dylib";
        macho::add_weak_dylib(&self.executable.inner.path, inject_path)?;
        sign::fakesign(&self.executable.inner.path)?;

        let mut count = 1; // main executable

        // Find all .appex plugins
        let plugins_dir = self.path.join("PlugIns");
        if plugins_dir.exists() {
            for entry in fs::read_dir(&plugins_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map(|e| e == "appex").unwrap_or(false) {
                    let plist_path = path.join("Info.plist");
                    if let Ok(pl) = PlistFile::open(&plist_path) {
                        if let Some(exec_name) = pl.get_string("CFBundleExecutable") {
                            let exec_path = path.join(exec_name);
                            if exec_path.exists() && macho::add_weak_dylib(&exec_path, inject_path).is_ok() {
                                sign::fakesign(&exec_path)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        // Also check Extensions directory (some apps use this)
        let extensions_dir = self.path.join("Extensions");
        if extensions_dir.exists() {
            for entry in fs::read_dir(&extensions_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map(|e| e == "appex").unwrap_or(false) {
                    let plist_path = path.join("Info.plist");
                    if let Ok(pl) = PlistFile::open(&plist_path) {
                        if let Some(exec_name) = pl.get_string("CFBundleExecutable") {
                            let exec_path = path.join(exec_name);
                            if exec_path.exists() && macho::add_weak_dylib(&exec_path, inject_path).is_ok() {
                                sign::fakesign(&exec_path)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("[*] patched \x1b[96m{}\x1b[0m executable(s) for plugin support", count);
        Ok(())
    }
}

fn delete_if_exists(path: &Path, bn: &str) -> bool {
    if path.exists() {
        let result = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };

        if result.is_ok() {
            println!("[?] {} already existed, replacing", bn);
            return true;
        }
    }
    false
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if ty.is_symlink() {
            let target = fs::read_link(&src_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &dst_path)?;
            #[cfg(windows)]
            {
                if src_path.is_dir() {
                    std::os::windows::fs::symlink_dir(target, &dst_path)?;
                } else {
                    std::os::windows::fs::symlink_file(target, &dst_path)?;
                }
            }
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
