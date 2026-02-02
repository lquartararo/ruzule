use crate::error::Result;
use std::fs;
use std::path::Path;

pub struct BundledFramework {
    pub name: &'static str,
    pub binary: &'static [u8],
    pub plist: &'static [u8],
}

pub static CYDIA_SUBSTRATE: BundledFramework = BundledFramework {
    name: "CydiaSubstrate",
    binary: include_bytes!("../frameworks/CydiaSubstrate"),
    plist: include_bytes!("../frameworks/CydiaSubstrate.plist"),
};

pub static ORION: BundledFramework = BundledFramework {
    name: "Orion",
    binary: include_bytes!("../frameworks/Orion"),
    plist: include_bytes!("../frameworks/Orion.plist"),
};

pub static CEPHEI: BundledFramework = BundledFramework {
    name: "Cephei",
    binary: include_bytes!("../frameworks/Cephei"),
    plist: include_bytes!("../frameworks/Cephei.plist"),
};

pub static CEPHEI_UI: BundledFramework = BundledFramework {
    name: "CepheiUI",
    binary: include_bytes!("../frameworks/CepheiUI"),
    plist: include_bytes!("../frameworks/CepheiUI.plist"),
};

pub static CEPHEI_PREFS: BundledFramework = BundledFramework {
    name: "CepheiPrefs",
    binary: include_bytes!("../frameworks/CepheiPrefs"),
    plist: include_bytes!("../frameworks/CepheiPrefs.plist"),
};

pub static ZX_PLUGINS_INJECT: &[u8] = include_bytes!("../frameworks/zxPluginsInject.dylib");

impl BundledFramework {
    pub fn framework_name(&self) -> String {
        format!("{}.framework", self.name)
    }

    pub fn extract_to<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest = dest.as_ref();
        let framework_dir = dest.join(self.framework_name());

        fs::create_dir_all(&framework_dir)?;
        fs::write(framework_dir.join(self.name), self.binary)?;
        fs::write(framework_dir.join("Info.plist"), self.plist)?;

        Ok(())
    }
}

pub fn get_framework_for_dep(dep_key: &str) -> Option<&'static BundledFramework> {
    match dep_key {
        "substrate." => Some(&CYDIA_SUBSTRATE),
        "orion." => Some(&ORION),
        "cephei." => Some(&CEPHEI),
        "cepheiui." => Some(&CEPHEI_UI),
        "cepheiprefs." => Some(&CEPHEI_PREFS),
        _ => None,
    }
}
