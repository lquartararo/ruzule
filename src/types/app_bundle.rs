use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::{Executable, PlistWrapper};
use crate::utils::tools::Tools;

pub struct AppBundle {
    pub path: PathBuf,
    pub plist: PlistWrapper,
    pub executable: Executable,
    cached_executables: Option<Vec<PathBuf>>,
}

impl AppBundle {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let plist_path = path.join("Info.plist");
        
        let plist = PlistWrapper::new(&plist_path, &path)?;
        
        let exec_name = plist
            .executable_name()
            .context("No CFBundleExecutable in Info.plist")?;
        let exec_path = path.join(exec_name);
        let executable = Executable::new(&exec_path)?;
        
        Ok(Self {
            path,
            plist,
            executable,
            cached_executables: None,
        })
    }

    pub fn remove<P: AsRef<Path>>(&self, name: P) -> bool {
        let name = name.as_ref();
        let path = if name.starts_with(&self.path) {
            name.to_path_buf()
        } else {
            self.path.join(name)
        };

        if !path.exists() {
            return false;
        }

        if path.is_dir() {
            fs::remove_dir_all(&path).is_ok()
        } else {
            fs::remove_file(&path).is_ok()
        }
    }

    pub fn remove_watch_apps(&mut self) {
        let removed = self.remove("Watch")
            | self.remove("WatchKit")
            | self.remove("com.apple.WatchPlaceholder");

        if removed {
            println!("[*] removed watch app");
        } else {
            println!("[?] watch app not present");
        }
    }

    fn get_executables(&self) -> Vec<PathBuf> {
        let mut executables = Vec::new();

        for entry in WalkDir::new(&self.path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "dylib" || ext_str == "appex" || ext_str == "framework" {
                    executables.push(path.to_path_buf());
                }
            }
        }

        executables
    }

    pub fn fakesign_all(&mut self, tools: &Tools) -> Result<()> {
        if self.cached_executables.is_none() {
            self.cached_executables = Some(self.get_executables());
        }

        let mut count = 0;

        if self.executable.fakesign(tools)? {
            count += 1;
        }

        for ts in self.cached_executables.as_ref().unwrap() {
            let success = if ts.extension().is_some_and(|e| e == "dylib") {
                Executable::new(ts)?.fakesign(tools)?
            } else {
                let pl = PlistWrapper::new(ts.join("Info.plist"), ts.clone())?;
                if let Some(exec_name) = pl.executable_name() {
                    Executable::new(ts.join(exec_name))?.fakesign(tools)?
                } else {
                    false
                }
            };

            if success {
                count += 1;
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

        if self.executable.thin()? {
            count += 1;
        }

        for ts in self.cached_executables.as_ref().unwrap() {
            let success = if ts.extension().is_some_and(|e| e == "dylib") {
                Executable::new(ts)?.thin()?
            } else {
                let pl = PlistWrapper::new(ts.join("Info.plist"), ts.clone())?;
                if let Some(exec_name) = pl.executable_name() {
                    Executable::new(ts.join(exec_name))?.thin()?
                } else {
                    false
                }
            };

            if success {
                count += 1;
            }
        }

        println!("[*] thinned \x1b[96m{}\x1b[0m item(s)", count);
        Ok(())
    }

    pub fn remove_all_extensions(&mut self) {
        let removed = self.remove("Extensions") | self.remove("PlugIns");

        if removed {
            println!("[*] removed app extensions");
        } else {
            println!("[?] no app extensions");
        }
    }

    pub fn remove_encrypted_extensions(&mut self, _tools: &Tools) -> Result<()> {
        let mut removed = Vec::new();
        
        let extensions_dir = self.path.join("PlugIns");
        let extensions_dir2 = self.path.join("Extensions");

        for dir in [extensions_dir, extensions_dir2] {
            if !dir.is_dir() {
                continue;
            }

            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().is_some_and(|e| e == "appex") {
                    if let Ok(bundle) = AppBundle::new(&path) {
                        if bundle.executable.is_encrypted()? {
                            self.remove(&path);
                            removed.push(bundle.executable.name);
                        }
                    }
                }
            }
        }

        if removed.is_empty() {
            println!("[?] no encrypted plugins");
        } else {
            println!("[*] removed encrypted plugins: {}", removed.join(", "));
        }

        Ok(())
    }

    pub fn inject(
        &mut self,
        tweaks: &mut HashMap<String, PathBuf>,
        tmpdir: &Path,
        tools: &Tools,
    ) -> Result<()> {
        let ent_path = self.path.join("ruzule.entitlements");
        let plugins_dir = self.path.join("PlugIns");
        let frameworks_dir = self.path.join("Frameworks");
        
        let has_entitlements = self.executable.write_entitlements(&ent_path, tools)?;
        self.executable.remove_signature(tools)?;

        if tweaks.values().any(|t| {
            t.extension().is_some_and(|e| e == "appex")
        }) {
            fs::create_dir_all(&plugins_dir)?;
        }

        if tweaks.values().any(|t| {
            let ext = t.extension().and_then(|e| e.to_str()).unwrap_or("");
            ext == "deb" || ext == "dylib" || ext == "framework"
        }) {
            fs::create_dir_all(&frameworks_dir)?;
            
            std::process::Command::new(&tools.install_name_tool)
                .args([
                    "-add_rpath",
                    "@executable_path/Frameworks",
                    self.executable.path.to_str().unwrap(),
                ])
                .stderr(std::process::Stdio::null())
                .output()?;
        }

        let orig_tweaks: Vec<_> = tweaks.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (bn, path) in orig_tweaks {
            if bn.ends_with(".deb") {
                crate::utils::deb::extract_deb(&path, tweaks, tmpdir)?;
            }
        }

        let mut needed: HashSet<String> = HashSet::new();

        for (bn, path) in tweaks.iter() {
            if path.is_symlink() {
                continue;
            }

            if bn.ends_with(".appex") {
                let fpath = plugins_dir.join(bn);
                delete_if_exists(&fpath, bn);
                copy_dir_recursive(path, &fpath)?;
            } else if bn.ends_with(".dylib") {
                let tmp_path = tmpdir.join(bn);
                fs::copy(path, &tmp_path)?;

                let exe = Executable::new(&tmp_path)?;
                exe.fix_common_dependencies(&mut needed, tools)?;
                exe.fix_dependencies(tweaks, tools)?;

                let fpath = frameworks_dir.join(bn);
                delete_if_exists(&fpath, bn);
                self.inject_dylib(&format!("@rpath/{}", bn), tools)?;
                fs::rename(&tmp_path, &fpath)?;
            } else if bn.ends_with(".framework") {
                let fpath = frameworks_dir.join(bn);
                delete_if_exists(&fpath, bn);
                let base = &bn[..bn.len() - 10];
                self.inject_dylib(&format!("@rpath/{}/{}", bn, base), tools)?;
                copy_dir_recursive(path, &fpath)?;
            } else if bn.ends_with(".bundle") {
                let fpath = self.path.join(bn);
                delete_if_exists(&fpath, bn);
                copy_dir_recursive(path, &fpath)?;
            } else {
                let fpath = self.path.join(bn);
                delete_if_exists(&fpath, bn);
                if path.is_dir() {
                    copy_dir_recursive(path, &fpath)?;
                } else {
                    fs::copy(path, &fpath)?;
                }
            }

            println!("[*] injected {}", bn);
        }

        if needed.contains("orion.") {
            needed.insert("substrate.".to_string());
        }

        for missing in &needed {
            if let Some(info) = crate::types::executable::COMMON_DEPS.get(missing.as_str()) {
                let real = info.name;
                let ip = frameworks_dir.join(real);
                let existed = delete_if_exists(&ip, real);
                let extras_path = tools.extras_dir.join(real);
                if extras_path.exists() {
                    copy_dir_recursive(&extras_path, &ip)?;
                    if !existed {
                        println!("[*] auto-injected {}", real);
                    }
                }
            }
        }

        if has_entitlements {
            self.executable.sign_with_entitlements(&ent_path, tools)?;
            println!("[*] restored entitlements");
        }

        Ok(())
    }

    fn inject_dylib(&self, cmd: &str, tools: &Tools) -> Result<()> {
        if let Some(ref insert_dylib) = tools.insert_dylib {
            let status = std::process::Command::new(insert_dylib)
                .args([
                    "--weak",
                    "--inplace",
                    "--all-yes",
                    cmd,
                    self.executable.path.to_str().unwrap(),
                ])
                .output()?;

            if !status.status.success() {
                anyhow::bail!(
                    "couldn't add LC (insert_dylib), error:\n{}",
                    String::from_utf8_lossy(&status.stderr)
                );
            }
        } else {
            anyhow::bail!("insert_dylib not available on this platform, cannot inject dylibs");
        }

        Ok(())
    }

    pub fn change_icon<P: AsRef<Path>>(&mut self, icon_path: P, tmpdir: &Path) -> Result<()> {
        use image::imageops::FilterType;

        use plist::{Dictionary, Value};

        let icon_path = icon_path.as_ref();
        let tmp_icon = tmpdir.join("icon.png");

        let img = image::open(icon_path)?;
        img.save(&tmp_icon)?;

        let uid = format!("ruzule_{:07x}a", rand::random::<u32>() & 0x0FFF_FFFF);
        let i60 = format!("{}60x60", uid);
        let i76 = format!("{}76x76", uid);

        let img = image::open(&tmp_icon)?;
        img.resize_exact(120, 120, FilterType::Lanczos3)
            .save(self.path.join(format!("{}@2x.png", i60)))?;
        img.resize_exact(152, 152, FilterType::Lanczos3)
            .save(self.path.join(format!("{}@2x~ipad.png", i76)))?;

        let mut primary_icon = Dictionary::new();
        primary_icon.insert(
            "CFBundleIconFiles".to_string(),
            Value::Array(vec![Value::String(i60.clone())]),
        );
        primary_icon.insert("CFBundleIconName".to_string(), Value::String(uid.clone()));

        let mut iphone_icons = Dictionary::new();
        iphone_icons.insert("CFBundlePrimaryIcon".to_string(), Value::Dictionary(primary_icon.clone()));

        let mut primary_icon_ipad = Dictionary::new();
        primary_icon_ipad.insert(
            "CFBundleIconFiles".to_string(),
            Value::Array(vec![Value::String(i60), Value::String(i76)]),
        );
        primary_icon_ipad.insert("CFBundleIconName".to_string(), Value::String(uid));

        let mut ipad_icons = Dictionary::new();
        ipad_icons.insert("CFBundlePrimaryIcon".to_string(), Value::Dictionary(primary_icon_ipad));

        self.plist.set("CFBundleIcons", Value::Dictionary(iphone_icons));
        self.plist.set("CFBundleIcons~ipad", Value::Dictionary(ipad_icons));
        self.plist.save()?;

        println!("[*] updated app icon");
        Ok(())
    }
}

fn delete_if_exists<P: AsRef<Path>>(path: P, bn: &str) -> bool {
    let path = path.as_ref();
    if path.exists() {
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
        println!("[?] {} already existed, replacing", bn);
        true
    } else {
        false
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

impl Executable {
    pub fn write_entitlements(&self, output: &Path, tools: &Tools) -> Result<bool> {
        let output_result = std::process::Command::new(&tools.ldid)
            .args(["-e", self.path.to_str().unwrap()])
            .output()?;

        if output_result.stdout.is_empty() {
            return Ok(false);
        }

        fs::write(output, &output_result.stdout)?;
        Ok(true)
    }

    pub fn sign_with_entitlements(&self, entitlements: &Path, tools: &Tools) -> Result<bool> {
        let status = std::process::Command::new(&tools.ldid)
            .args([
                &format!("-S{}", entitlements.display()),
                "-M",
                self.path.to_str().unwrap(),
            ])
            .status()?;
        Ok(status.success())
    }

    pub fn merge_entitlements(&self, entitlements: &Path, tools: &Tools) -> Result<()> {
        if self.sign_with_entitlements(entitlements, tools)? {
            println!("[*] merged new entitlements");
        } else {
            println!("[!] failed to merge new entitlements, are they valid?");
        }
        Ok(())
    }
}
