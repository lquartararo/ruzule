use anyhow::{bail, Context, Result};
use goblin::mach::{Mach, MachO};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::utils::tools::Tools;

pub struct Executable {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Clone)]
pub struct DependencyInfo {
    pub name: &'static str,
    pub path: &'static str,
}

lazy_static::lazy_static! {
    pub static ref COMMON_DEPS: HashMap<&'static str, DependencyInfo> = {
        let mut m = HashMap::new();
        m.insert("substrate.", DependencyInfo {
            name: "CydiaSubstrate.framework",
            path: "@rpath/CydiaSubstrate.framework/CydiaSubstrate",
        });
        m.insert("orion.", DependencyInfo {
            name: "Orion.framework",
            path: "@rpath/Orion.framework/Orion",
        });
        m.insert("cephei.", DependencyInfo {
            name: "Cephei.framework",
            path: "@rpath/Cephei.framework/Cephei",
        });
        m.insert("cepheiui.", DependencyInfo {
            name: "CepheiUI.framework",
            path: "@rpath/CepheiUI.framework/CepheiUI",
        });
        m.insert("cepheiprefs.", DependencyInfo {
            name: "CepheiPrefs.framework",
            path: "@rpath/CepheiPrefs.framework/CepheiPrefs",
        });
        m
    };
}

const DEP_STARTERS: [&str; 3] = ["/Library/", "/usr/lib/", "@"];

impl Executable {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.is_file() {
            bail!(
                "{} does not exist (executable)\n\
                [?] check the wiki for info: \
                https://github.com/asdfzxcvbn/pyzule-rw/wiki/file-does-not-exist-(executable)-%3F",
                path.display()
            );
        }

        let name = path
            .file_name()
            .context("Invalid executable path")?
            .to_string_lossy()
            .to_string();

        Ok(Self { path, name })
    }

    pub fn is_encrypted(&self) -> Result<bool> {
        let data = fs::read(&self.path)?;
        
        match goblin::mach::Mach::parse(&data)? {
            Mach::Binary(macho) => Ok(Self::check_encryption(&macho)),
            Mach::Fat(fat) => {
                for arch in fat.iter_arches().flatten() {
                    let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                    if let Ok(macho) = MachO::parse(slice, 0) {
                        if Self::check_encryption(&macho) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
        }
    }

    fn check_encryption(macho: &MachO) -> bool {
        use goblin::mach::load_command::CommandVariant;
        
        for cmd in &macho.load_commands {
            match cmd.command {
                CommandVariant::EncryptionInfo32(info) => {
                    if info.cryptid != 0 {
                        return true;
                    }
                }
                CommandVariant::EncryptionInfo64(info) => {
                    if info.cryptid != 0 {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn remove_signature(&self, tools: &Tools) -> Result<()> {
        Command::new(&tools.ldid)
            .args(["-R", self.path.to_str().unwrap()])
            .output()?;
        Ok(())
    }

    pub fn fakesign(&self, tools: &Tools) -> Result<bool> {
        let status = Command::new(&tools.ldid)
            .args(["-S", "-M", self.path.to_str().unwrap()])
            .status()?;
        Ok(status.success())
    }

    pub fn thin(&self) -> Result<bool> {
        use goblin::mach::cputype::CPU_TYPE_ARM64;
        
        let data = fs::read(&self.path)?;
        
        match Mach::parse(&data)? {
            Mach::Binary(macho) => {
                if macho.header.cputype() == CPU_TYPE_ARM64 {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Mach::Fat(fat) => {
                for arch in fat.iter_arches().flatten() {
                    if arch.cputype() == CPU_TYPE_ARM64 {
                        let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                        fs::write(&self.path, slice)?;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    pub fn change_dependency(&self, old: &str, new: &str, tools: &Tools) -> Result<()> {
        Command::new(&tools.install_name_tool)
            .args(["-change", old, new, self.path.to_str().unwrap()])
            .stderr(std::process::Stdio::null())
            .output()?;
        Ok(())
    }

    pub fn get_dependencies(&self) -> Result<Vec<String>> {
        let data = fs::read(&self.path)?;
        let mut deps = Vec::new();
        
        match Mach::parse(&data)? {
            Mach::Binary(macho) => {
                Self::collect_dependencies(&macho, &mut deps);
            }
            Mach::Fat(fat) => {
                for arch in fat.iter_arches().flatten() {
                    let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                    if let Ok(macho) = MachO::parse(slice, 0) {
                        Self::collect_dependencies(&macho, &mut deps);
                        break;
                    }
                }
            }
        }
        
        Ok(deps)
    }
    
    fn collect_dependencies(macho: &MachO, deps: &mut Vec<String>) {
        use goblin::mach::load_command::CommandVariant;
        
        for cmd in &macho.load_commands {
            let name = match &cmd.command {
                CommandVariant::LoadDylib(dylib)
                | CommandVariant::LoadWeakDylib(dylib)
                | CommandVariant::ReexportDylib(dylib)
                | CommandVariant::LazyLoadDylib(dylib) => {
                    Some(dylib.dylib.name.to_string())
                }
                _ => None,
            };
            
            if let Some(name) = name {
                if DEP_STARTERS.iter().any(|s| name.starts_with(s)) && !deps.contains(&name) {
                    deps.push(name);
                }
            }
        }
    }

    pub fn fix_common_dependencies(
        &self,
        needed: &mut std::collections::HashSet<String>,
        tools: &Tools,
    ) -> Result<()> {
        self.remove_signature(tools)?;
        
        for dep in self.get_dependencies()? {
            let dep_lower = dep.to_lowercase();
            for (common, info) in COMMON_DEPS.iter() {
                if dep_lower.contains(common) {
                    needed.insert(common.to_string());
                    
                    if dep != info.path {
                        self.change_dependency(&dep, info.path, tools)?;
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

    pub fn fix_dependencies(
        &self,
        tweaks: &HashMap<String, PathBuf>,
        tools: &Tools,
    ) -> Result<()> {
        for dep in self.get_dependencies()? {
            for cname in tweaks.keys() {
                if dep.contains(cname) {
                    let npath = if cname.ends_with(".framework") {
                        let base = &cname[..cname.len() - 10];
                        format!("@rpath/{}/{}", cname, base)
                    } else {
                        format!("@rpath/{}", cname)
                    };
                    
                    if dep != npath {
                        self.change_dependency(&dep, &npath, tools)?;
                        println!("[*] fixed dependency in {}: {} -> {}", self.name, dep, npath);
                    }
                }
            }
        }
        
        Ok(())
    }
}
