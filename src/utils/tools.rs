use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Tools {
    pub ldid: PathBuf,
    pub install_name_tool: PathBuf,
    pub insert_dylib: Option<PathBuf>,
    pub extras_dir: PathBuf,
}

impl Tools {
    pub fn new() -> Result<Self> {
        let (system, machine) = Self::get_platform();
        
        let exe_path = std::env::current_exe()?;
        let install_dir = exe_path.parent().unwrap().to_path_buf();
        let tools_dir = install_dir.join("tools").join(&system).join(&machine);
        
        let has_bundled_tools = tools_dir.is_dir();
        
        // Try bundled tools first, fall back to system tools
        let ldid = Self::find_tool(&tools_dir, &install_dir, "ldid", has_bundled_tools);
        let install_name_tool = Self::find_tool(&tools_dir, &install_dir, "install_name_tool", has_bundled_tools);
        
        let insert_dylib = Self::find_insert_dylib(&tools_dir, &install_dir);
        
        let extras_dir = install_dir.join("extras");
        
        Ok(Self {
            ldid,
            install_name_tool,
            insert_dylib,
            extras_dir,
        })
    }

    fn find_tool(tools_dir: &Path, install_dir: &Path, name: &str, has_bundled: bool) -> PathBuf {
        // Check platform-specific tools dir first
        if has_bundled {
            let bundled = tools_dir.join(name);
            if bundled.is_file() {
                return bundled;
            }
        }
        
        // Check tools/ directly next to executable
        let simple = install_dir.join("tools").join(name);
        if simple.is_file() {
            return simple;
        }
        
        // Check current working directory tools/
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_tools = cwd.join("tools").join(name);
            if cwd_tools.is_file() {
                return cwd_tools;
            }
        }
        
        // Fall back to system tool
        Self::which(name)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(name))
    }

    fn which(name: &str) -> Option<String> {
        Command::new("which")
            .arg(name)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    fn get_platform() -> (String, String) {
        let system = std::env::consts::OS.to_string();
        let system = match system.as_str() {
            "linux" => "Linux",
            "macos" => "Darwin",
            _ => &system,
        }.to_string();
        
        let machine = std::env::consts::ARCH.to_string();
        let machine = match machine.as_str() {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            _ => &machine,
        }.to_string();
        
        (system, machine)
    }

    pub fn check_required(&self, need_signing: bool, need_injection: bool) -> Result<()> {
        if need_signing && !self.tool_available(&self.ldid) {
            anyhow::bail!("ldid not found - required for signing operations. Install it or provide bundled tools.");
        }
        if need_injection && self.insert_dylib.is_none() {
            anyhow::bail!("insert_dylib not found - required for dylib injection. Install it or provide bundled tools.");
        }
        Ok(())
    }

    fn tool_available(&self, path: &Path) -> bool {
        path.is_file() || Command::new(path).arg("--version").output().is_ok()
    }

    fn find_insert_dylib(tools_dir: &Path, install_dir: &Path) -> Option<PathBuf> {
        // Check platform-specific tools dir first
        let bundled = tools_dir.join("insert_dylib");
        if bundled.is_file() {
            return Some(bundled);
        }
        
        // Check tools/ directly next to executable
        let simple = install_dir.join("tools").join("insert_dylib");
        if simple.is_file() {
            return Some(simple);
        }
        
        // Check current working directory tools/
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_tools = cwd.join("tools").join("insert_dylib");
            if cwd_tools.is_file() {
                return Some(cwd_tools);
            }
        }
        
        // Fall back to PATH
        Self::which("insert_dylib").map(PathBuf::from)
    }
}
