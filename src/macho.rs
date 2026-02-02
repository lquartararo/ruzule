use crate::error::{Result, RuzuleError};
use apple_codesign::{MachFile, MachOBinary, UniversalBinaryBuilder};
use goblin::mach::cputype::CPU_TYPE_ARM64;
use goblin::mach::load_command::{
    CommandVariant, LC_ID_DYLIB, LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB, LC_REEXPORT_DYLIB,
    LC_LAZY_LOAD_DYLIB, LC_LOAD_UPWARD_DYLIB, LC_RPATH,
};
use goblin::mach::Mach;
use goblin::mach::MachO as GoblinMachO;
use std::fs;
use std::path::Path;

const DYLIB_COMMANDS: &[u32] = &[
    LC_LOAD_DYLIB,
    LC_LOAD_WEAK_DYLIB,
    LC_REEXPORT_DYLIB,
    LC_LAZY_LOAD_DYLIB,
    LC_LOAD_UPWARD_DYLIB,
];

pub trait MachOExt {
    fn add_dylib_load_path(&mut self, path: &str) -> Result<()>;
    fn replace_dylib_load_path(&mut self, old_path: &str, new_path: &str) -> Result<()>;
    fn replace_install_name(&mut self, new_name: &str) -> Result<()>;
    fn add_rpath(&mut self, path: &str) -> Result<()>;
}

impl MachOExt for MachOBinary<'_> {
    fn add_dylib_load_path(&mut self, path: &str) -> Result<()> {
        let macho = &self.macho;

        let read_u32_le = |data: &[u8], offset: usize| -> u32 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        let dylib_exists_in_macho = |macho: &GoblinMachO, base_offset: usize| -> bool {
            macho.load_commands.iter().any(|load_cmd| {
                if let CommandVariant::LoadDylib(dylib) = &load_cmd.command {
                    extract_dylib_path(self.data, base_offset + load_cmd.offset, dylib.dylib.name)
                        .is_some_and(|name| name == path)
                } else {
                    manually_parse_dylib(self.data, base_offset + load_cmd.offset)
                        .is_some_and(|name| name == path)
                }
            })
        };

        let is_64 = matches!(macho.header.cputype, CPU_TYPE_ARM64);
        let dylib_exists = dylib_exists_in_macho(macho, 0);
        let current_sizeofcmds = read_u32_le(self.data, 20);
        let current_ncmds = read_u32_le(self.data, 16);

        let mut data = self.data.to_vec();

        if dylib_exists {
            eprintln!("[?] Dylib already exists in binary: {}", path);
            return Ok(());
        }

        let header_size = if is_64 { 32 } else { 28 };

        let dylib_path_len = path.len();
        let padding = (8 - ((dylib_path_len + 1) % 8)) % 8;
        let dylib_command_size = 24 + dylib_path_len + 1 + padding;

        let load_commands_offset = header_size;
        let sizeofcmds_offset = 20;
        let ncmds_offset = 16;

        let min_fileoff = macho
            .load_commands
            .iter()
            .filter_map(|load_cmd| match &load_cmd.command {
                CommandVariant::Segment64(seg) if seg.filesize > 0 && seg.fileoff > 0 => {
                    Some(seg.fileoff)
                }
                CommandVariant::Segment32(seg) if seg.filesize > 0 && seg.fileoff > 0 => {
                    Some(seg.fileoff as u64)
                }
                _ => None,
            })
            .min()
            .unwrap_or(u64::MAX);

        let load_commands_end = load_commands_offset + current_sizeofcmds as usize;
        let data_start = if min_fileoff < u64::MAX {
            min_fileoff as usize
        } else {
            data.len()
        };

        let available_space = data_start.saturating_sub(load_commands_end);

        if dylib_command_size > available_space {
            return Err(RuzuleError::MachO(format!(
                "Not enough space for new load command (need {}, have {})",
                dylib_command_size, available_space
            )));
        }

        let insert_offset = load_commands_end;
        let mut new_command = Vec::new();
        new_command.extend_from_slice(&LC_LOAD_WEAK_DYLIB.to_le_bytes());
        new_command.extend_from_slice(&(dylib_command_size as u32).to_le_bytes());
        new_command.extend_from_slice(&24u32.to_le_bytes());
        new_command.extend_from_slice(&2u32.to_le_bytes());
        new_command.extend_from_slice(&0x00010000u32.to_le_bytes());
        new_command.extend_from_slice(&0x00010000u32.to_le_bytes());
        new_command.extend_from_slice(path.as_bytes());
        new_command.push(0);
        new_command.extend(vec![0u8; padding]);

        data[insert_offset..insert_offset + dylib_command_size].copy_from_slice(&new_command);

        let new_sizeofcmds = current_sizeofcmds + dylib_command_size as u32;
        let new_ncmds = current_ncmds + 1;

        data[sizeofcmds_offset..sizeofcmds_offset + 4]
            .copy_from_slice(&new_sizeofcmds.to_le_bytes());
        data[ncmds_offset..ncmds_offset + 4].copy_from_slice(&new_ncmds.to_le_bytes());

        self.data = Box::leak(data.into_boxed_slice());

        Ok(())
    }

    fn replace_dylib_load_path(&mut self, old_path: &str, new_path: &str) -> Result<()> {
        let macho = &self.macho;
        let mut data = self.data.to_vec();

        let read_u32_le = |data: &[u8], offset: usize| -> u32 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        let find_dylib_matches = |macho: &GoblinMachO, base_offset: usize| -> Vec<(usize, usize)> {
            macho
                .load_commands
                .iter()
                .filter(|load_cmd| DYLIB_COMMANDS.contains(&load_cmd.command.cmd()))
                .filter_map(|load_cmd| {
                    let path_found = match &load_cmd.command {
                        CommandVariant::LoadDylib(dylib) => {
                            extract_dylib_path(
                                self.data,
                                base_offset + load_cmd.offset,
                                dylib.dylib.name,
                            )
                        }
                        _ => manually_parse_dylib(self.data, base_offset + load_cmd.offset),
                    }?;

                    if path_found == old_path {
                        let cmdsize =
                            read_u32_le(self.data, base_offset + load_cmd.offset + 4) as usize;
                        return Some((load_cmd.offset, cmdsize));
                    }
                    None
                })
                .collect()
        };

        let replacements: Vec<(usize, usize, usize)> = find_dylib_matches(macho, 0)
            .into_iter()
            .map(|(offset, size)| (0, offset, size))
            .collect();

        if replacements.is_empty() {
            return Ok(());
        }

        for (arch_offset, cmd_offset, cmdsize) in &replacements {
            let absolute_cmd_offset = arch_offset + cmd_offset;
            let dylib_name_offset = absolute_cmd_offset + 24;
            let available_space = cmdsize - 24;

            let new_path_len = new_path.len();
            let old_path_len = old_path.len();
            let new_padding = (8 - ((new_path_len + 1) % 8)) % 8;
            let required_space = new_path_len + 1 + new_padding;

            if required_space > available_space {
                return Err(RuzuleError::MachO(
                    "Not enough space for new dylib path".to_string(),
                ));
            }

            let old_padding = (8 - ((old_path_len + 1) % 8)) % 8;
            let old_total_size = old_path_len + 1 + old_padding;
            for i in 0..old_total_size.min(available_space) {
                data[dylib_name_offset + i] = 0;
            }

            data[dylib_name_offset..dylib_name_offset + new_path_len]
                .copy_from_slice(new_path.as_bytes());
        }

        self.data = Box::leak(data.into_boxed_slice());

        Ok(())
    }

    fn replace_install_name(&mut self, new_name: &str) -> Result<()> {
        let macho = &self.macho;
        let mut data = self.data.to_vec();

        let read_u32_le = |data: &[u8], offset: usize| -> u32 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        // Find LC_ID_DYLIB command
        for load_cmd in &macho.load_commands {
            if load_cmd.command.cmd() == LC_ID_DYLIB {
                let cmd_offset = load_cmd.offset;
                let cmdsize = read_u32_le(self.data, cmd_offset + 4) as usize;

                // Get old name for calculating space
                let old_name = match &load_cmd.command {
                    CommandVariant::IdDylib(dylib) => {
                        extract_dylib_path(self.data, cmd_offset, dylib.dylib.name)
                    }
                    _ => manually_parse_dylib(self.data, cmd_offset),
                };

                let dylib_name_offset = cmd_offset + 24;
                let available_space = cmdsize - 24;

                let new_name_len = new_name.len();
                let new_padding = (8 - ((new_name_len + 1) % 8)) % 8;
                let required_space = new_name_len + 1 + new_padding;

                if required_space > available_space {
                    return Err(RuzuleError::MachO(
                        "Not enough space for new install name".to_string(),
                    ));
                }

                // Zero out old name
                if let Some(old) = old_name {
                    let old_len = old.len();
                    let old_padding = (8 - ((old_len + 1) % 8)) % 8;
                    let old_total_size = old_len + 1 + old_padding;
                    for i in 0..old_total_size.min(available_space) {
                        data[dylib_name_offset + i] = 0;
                    }
                }

                // Write new name
                data[dylib_name_offset..dylib_name_offset + new_name_len]
                    .copy_from_slice(new_name.as_bytes());

                break;
            }
        }

        self.data = Box::leak(data.into_boxed_slice());

        Ok(())
    }

    fn add_rpath(&mut self, path: &str) -> Result<()> {
        let macho = &self.macho;

        let read_u32_le = |data: &[u8], offset: usize| -> u32 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        // Check if rpath already exists
        let rpath_exists = macho.load_commands.iter().any(|load_cmd| {
            if load_cmd.command.cmd() == LC_RPATH {
                // Parse the rpath path from the load command
                let path_offset = load_cmd.offset + 8; // rpath_command has cmd(4) + cmdsize(4) + path offset(4)
                if path_offset + 4 <= self.data.len() {
                    let name_offset = read_u32_le(self.data, path_offset);
                    if let Some(existing) = extract_rpath(self.data, load_cmd.offset, name_offset) {
                        return existing == path;
                    }
                }
            }
            false
        });

        if rpath_exists {
            return Ok(());
        }

        let is_64 = matches!(macho.header.cputype, CPU_TYPE_ARM64);
        let current_sizeofcmds = read_u32_le(self.data, 20);
        let current_ncmds = read_u32_le(self.data, 16);

        let mut data = self.data.to_vec();

        let header_size = if is_64 { 32 } else { 28 };

        // Calculate new load command size (must be 8-byte aligned)
        // rpath_command: cmd(4) + cmdsize(4) + path_offset(4) = 12 bytes header
        let rpath_len = path.len();
        let padding = (8 - ((rpath_len + 1) % 8)) % 8;
        let rpath_command_size = 12 + rpath_len + 1 + padding;

        let load_commands_offset = header_size;
        let sizeofcmds_offset = 20;
        let ncmds_offset = 16;

        // Find the minimum non-zero file offset from segments
        let min_fileoff = macho
            .load_commands
            .iter()
            .filter_map(|load_cmd| match &load_cmd.command {
                CommandVariant::Segment64(seg) if seg.filesize > 0 && seg.fileoff > 0 => {
                    Some(seg.fileoff)
                }
                CommandVariant::Segment32(seg) if seg.filesize > 0 && seg.fileoff > 0 => {
                    Some(seg.fileoff as u64)
                }
                _ => None,
            })
            .min()
            .unwrap_or(u64::MAX);

        let load_commands_end = load_commands_offset + current_sizeofcmds as usize;
        let data_start = if min_fileoff < u64::MAX {
            min_fileoff as usize
        } else {
            data.len()
        };

        let available_space = data_start.saturating_sub(load_commands_end);

        if rpath_command_size > available_space {
            return Err(RuzuleError::MachO(format!(
                "Not enough space for new rpath command (need {}, have {})",
                rpath_command_size, available_space
            )));
        }

        let insert_offset = load_commands_end;
        let mut new_command = Vec::new();
        new_command.extend_from_slice(&LC_RPATH.to_le_bytes());
        new_command.extend_from_slice(&(rpath_command_size as u32).to_le_bytes());
        new_command.extend_from_slice(&12u32.to_le_bytes()); // path offset from start of command
        new_command.extend_from_slice(path.as_bytes());
        new_command.push(0);
        new_command.extend(vec![0u8; padding]);

        data[insert_offset..insert_offset + rpath_command_size].copy_from_slice(&new_command);

        let new_sizeofcmds = current_sizeofcmds + rpath_command_size as u32;
        let new_ncmds = current_ncmds + 1;

        data[sizeofcmds_offset..sizeofcmds_offset + 4]
            .copy_from_slice(&new_sizeofcmds.to_le_bytes());
        data[ncmds_offset..ncmds_offset + 4].copy_from_slice(&new_ncmds.to_le_bytes());

        self.data = Box::leak(data.into_boxed_slice());

        Ok(())
    }
}

fn extract_rpath(file_data: &[u8], load_cmd_offset: usize, name_offset: u32) -> Option<String> {
    let name_offset = load_cmd_offset + name_offset as usize;
    if name_offset >= file_data.len() {
        return None;
    }

    let mut end = name_offset;
    while end < file_data.len() && file_data[end] != 0 {
        end += 1;
    }

    std::str::from_utf8(&file_data[name_offset..end])
        .ok()
        .map(|s| s.to_string())
}

fn extract_dylib_path(
    file_data: &[u8],
    load_cmd_offset: usize,
    name_offset_rel: u32,
) -> Option<String> {
    let name_offset = load_cmd_offset + name_offset_rel as usize;
    if name_offset >= file_data.len() {
        return None;
    }

    let mut end = name_offset;
    while end < file_data.len() && file_data[end] != 0 {
        end += 1;
    }

    std::str::from_utf8(&file_data[name_offset..end])
        .ok()
        .map(|s| s.to_string())
}

fn manually_parse_dylib(file_data: &[u8], load_cmd_offset: usize) -> Option<String> {
    if load_cmd_offset + 12 > file_data.len() {
        return None;
    }

    let name_offset_field = u32::from_le_bytes([
        file_data[load_cmd_offset + 8],
        file_data[load_cmd_offset + 9],
        file_data[load_cmd_offset + 10],
        file_data[load_cmd_offset + 11],
    ]);

    extract_dylib_path(file_data, load_cmd_offset, name_offset_field)
}

pub fn is_encrypted<P: AsRef<Path>>(path: P) -> Result<bool> {
    let data = fs::read(path.as_ref())?;

    match Mach::parse(&data)? {
        Mach::Binary(macho) => Ok(check_encrypted_goblin(&macho)),
        Mach::Fat(fat) => {
            for arch in fat.iter_arches() {
                let arch = arch?;
                let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                if let Ok(macho) = goblin::mach::MachO::parse(slice, 0) {
                    if check_encrypted_goblin(&macho) {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
    }
}

fn check_encrypted_goblin(macho: &GoblinMachO) -> bool {
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

pub fn get_dependencies<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let data = fs::read(path.as_ref())?;
    let mut deps = Vec::new();

    match Mach::parse(&data)? {
        Mach::Binary(macho) => {
            collect_deps_goblin(&macho, &mut deps);
        }
        Mach::Fat(fat) => {
            for arch in fat.iter_arches() {
                let arch = arch?;
                let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                if let Ok(macho) = goblin::mach::MachO::parse(slice, 0) {
                    collect_deps_goblin(&macho, &mut deps);
                    break;
                }
            }
        }
    }

    let filtered: Vec<String> = deps
        .into_iter()
        .filter(|d| {
            d.starts_with("/Library/")
                || d.starts_with("/usr/lib/")
                || d.starts_with("@")
        })
        .collect();

    Ok(filtered)
}

fn collect_deps_goblin(macho: &GoblinMachO, deps: &mut Vec<String>) {
    for lib in &macho.libs {
        if !lib.is_empty() {
            deps.push(lib.to_string());
        }
    }
}

pub fn add_weak_dylib<P: AsRef<Path>>(path: P, dylib_path: &str) -> Result<()> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mut mach_file = MachFile::parse(data)
        .map_err(|e| RuzuleError::MachO(format!("Failed to parse Mach-O: {}", e)))?;

    for macho in mach_file.iter_macho_mut() {
        macho.add_dylib_load_path(dylib_path)?;
    }

    write_mach_file(&mach_file, path)?;
    Ok(())
}

pub fn replace_dylib<P: AsRef<Path>>(path: P, old_path: &str, new_path: &str) -> Result<()> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mut mach_file = MachFile::parse(data)
        .map_err(|e| RuzuleError::MachO(format!("Failed to parse Mach-O: {}", e)))?;

    for macho in mach_file.iter_macho_mut() {
        macho.replace_dylib_load_path(old_path, new_path)?;
    }

    write_mach_file(&mach_file, path)?;
    Ok(())
}

pub fn change_install_name<P: AsRef<Path>>(path: P, new_name: &str) -> Result<()> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mut mach_file = MachFile::parse(data)
        .map_err(|e| RuzuleError::MachO(format!("Failed to parse Mach-O: {}", e)))?;

    for macho in mach_file.iter_macho_mut() {
        macho.replace_install_name(new_name)?;
    }

    write_mach_file(&mach_file, path)?;
    Ok(())
}

pub fn add_rpath<P: AsRef<Path>>(path: P, rpath: &str) -> Result<()> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mut mach_file = MachFile::parse(data)
        .map_err(|e| RuzuleError::MachO(format!("Failed to parse Mach-O: {}", e)))?;

    for macho in mach_file.iter_macho_mut() {
        macho.add_rpath(rpath)?;
    }

    write_mach_file(&mach_file, path)?;
    Ok(())
}

fn write_mach_file(mach_file: &MachFile, path: &Path) -> Result<()> {
    let mut builder = UniversalBinaryBuilder::default();
    for binary in mach_file.iter_macho() {
        let _ = builder.add_binary(binary.data);
    }

    let mut file = fs::File::create(path)?;
    builder.write(&mut file)
        .map_err(|e| RuzuleError::MachO(format!("Failed to write Mach-O: {}", e)))?;

    Ok(())
}

pub fn thin_to_arm64<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();
    let data = fs::read(path)?;

    match Mach::parse(&data)? {
        Mach::Binary(macho) => {
            let cputype = macho.header.cputype();
            if cputype == CPU_TYPE_ARM64 {
                Ok(false)
            } else {
                Err(RuzuleError::MachO("Binary is not arm64".to_string()))
            }
        }
        Mach::Fat(fat) => {
            for arch in fat.iter_arches() {
                let arch = arch?;
                if arch.cputype() == CPU_TYPE_ARM64 {
                    let slice = &data[arch.offset as usize..(arch.offset + arch.size) as usize];
                    fs::write(path, slice)?;
                    return Ok(true);
                }
            }
            Err(RuzuleError::MachO("No arm64 slice found in fat binary".to_string()))
        }
    }
}

pub fn remove_code_signature<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let data = Box::leak(data.into_boxed_slice());

    let mach_file = MachFile::parse(data)
        .map_err(|e| RuzuleError::MachO(format!("Failed to parse Mach-O: {}", e)))?;

    write_mach_file(&mach_file, path)?;
    Ok(())
}
