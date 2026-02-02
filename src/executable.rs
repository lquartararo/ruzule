use crate::error::{Result, RuzuleError};
use crate::macho;
use crate::sign;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct CommonDep {
    pub name: &'static str,
    pub path: &'static str,
}

pub static COMMON_DEPS: LazyLock<HashMap<&'static str, CommonDep>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("substrate.", CommonDep {
        name: "CydiaSubstrate.framework",
        path: "@rpath/CydiaSubstrate.framework/CydiaSubstrate",
    });
    m.insert("orion.", CommonDep {
        name: "Orion.framework",
        path: "@rpath/Orion.framework/Orion",
    });
    m.insert("cephei.", CommonDep {
        name: "Cephei.framework",
        path: "@rpath/Cephei.framework/Cephei",
    });
    m.insert("cepheiui.", CommonDep {
        name: "CepheiUI.framework",
        path: "@rpath/CepheiUI.framework/CepheiUI",
    });
    m.insert("cepheiprefs.", CommonDep {
        name: "CepheiPrefs.framework",
        path: "@rpath/CepheiPrefs.framework/CepheiPrefs",
    });
    m
});

pub struct Executable {
    pub path: PathBuf,
    pub name: String,
}

impl Executable {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(RuzuleError::FileNotFound(path));
        }

        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(Self { path, name })
    }

    pub fn is_encrypted(&self) -> Result<bool> {
        macho::is_encrypted(&self.path)
    }

    pub fn remove_signature(&self) -> Result<()> {
        sign::remove_signature(&self.path)
    }

    pub fn fakesign(&self) -> Result<bool> {
        sign::fakesign(&self.path)
    }

    pub fn thin(&self) -> Result<bool> {
        macho::thin_to_arm64(&self.path)
    }

    pub fn get_dependencies(&self) -> Result<Vec<String>> {
        macho::get_dependencies(&self.path)
    }

    pub fn change_dependency(&self, old: &str, new: &str) -> Result<()> {
        macho::replace_dylib(&self.path, old, new)
    }

    pub fn change_install_name(&self, new_name: &str) -> Result<()> {
        macho::change_install_name(&self.path, new_name)
    }

    pub fn fix_common_dependencies(&self, needed: &mut HashSet<String>) -> Result<()> {
        self.remove_signature()?;

        let deps = self.get_dependencies()?;
        for dep in deps {
            let dep_lower = dep.to_lowercase();
            for (key, info) in COMMON_DEPS.iter() {
                if dep_lower.contains(key) {
                    needed.insert(key.to_string());

                    if dep != info.path {
                        self.change_dependency(&dep, info.path)?;
                        println!(
                            "[*] fixed common dependency in {}: {} -> {}",
                            self.name, dep, info.path
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn fix_dependencies(&self, tweaks: &HashMap<String, PathBuf>) -> Result<()> {
        let deps = self.get_dependencies()?;

        for dep in deps {
            for cname in tweaks.keys() {
                if dep.contains(cname) {
                    let npath = if cname.ends_with(".framework") {
                        let framework_name = cname.strip_suffix(".framework").unwrap();
                        format!("@rpath/{}/{}", cname, framework_name)
                    } else {
                        format!("@rpath/{}", cname)
                    };

                    if dep != npath {
                        self.change_dependency(&dep, &npath)?;
                        println!("[*] fixed dependency in {}: {} -> {}", self.name, dep, npath);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn fix_install_name(&self, tweaks: &HashMap<String, PathBuf>) -> Result<()> {
        // Fix install name (LC_ID_DYLIB) for dylibs
        for cname in tweaks.keys() {
            if self.name == *cname {
                let npath = if cname.ends_with(".framework") {
                    let framework_name = cname.strip_suffix(".framework").unwrap();
                    format!("@rpath/{}/{}", cname, framework_name)
                } else {
                    format!("@rpath/{}", cname)
                };
                self.change_install_name(&npath)?;
                println!("[*] fixed install name for {}: -> {}", self.name, npath);
                break;
            }
        }
        Ok(())
    }
}

pub struct MainExecutable {
    pub inner: Executable,
    pub bundle_path: PathBuf,
}

impl MainExecutable {
    pub fn new<P: AsRef<Path>>(path: P, bundle_path: P) -> Result<Self> {
        let inner = Executable::new(path)?;
        Ok(Self {
            inner,
            bundle_path: bundle_path.as_ref().to_path_buf(),
        })
    }

    pub fn is_encrypted(&self) -> Result<bool> {
        self.inner.is_encrypted()
    }

    pub fn fakesign(&self) -> Result<bool> {
        self.inner.fakesign()
    }

    pub fn thin(&self) -> Result<bool> {
        self.inner.thin()
    }

    pub fn add_rpath(&self, rpath: &str) -> Result<()> {
        macho::add_rpath(&self.inner.path, rpath)
    }

    pub fn inject_dylib(&self, dylib_path: &str) -> Result<()> {
        macho::add_weak_dylib(&self.inner.path, dylib_path)
    }

    pub fn write_entitlements<P: AsRef<Path>>(&self, output: P) -> Result<bool> {
        let ent_data = sign::extract_entitlements(&self.inner.path)?;
        if ent_data.is_empty() {
            return Ok(false);
        }
        std::fs::write(output, ent_data)?;
        Ok(true)
    }

    pub fn sign_with_entitlements<P: AsRef<Path>>(&self, entitlements: P) -> Result<bool> {
        sign::sign_with_entitlements(&self.inner.path, entitlements)
    }

    pub fn merge_entitlements<P: AsRef<Path>>(&self, entitlements: P) -> Result<()> {
        if self.sign_with_entitlements(entitlements)? {
            println!("[*] merged new entitlements");
        } else {
            println!("[!] failed to merge new entitlements, are they valid?");
        }
        Ok(())
    }
}
