pub mod cyan;
pub mod deb;
pub mod ipa;
pub mod tools;

use crate::cli::Args;
use std::path::Path;

pub fn validate_inputs(args: &Args) -> Option<String> {
    let input = &args.input;
    
    if !input.ends_with(".app") && !input.ends_with(".ipa") && !input.ends_with(".tipa") {
        return Some("the input file must be an ipa/tipa/app".to_string());
    }

    if !Path::new(input).exists() {
        return Some(format!("{} does not exist", input));
    }

    let output = args.get_output();
    if Path::new(&output).exists() && !args.overwrite {
        if output == args.input {
            eprint!("[<] no output was specified. overwrite the input? [Y/n] ");
        } else {
            eprint!("[<] {} already exists, overwrite it? [Y/n] ", output);
        }
        
        let mut input_str = String::new();
        if std::io::stdin().read_line(&mut input_str).is_ok() {
            let trimmed = input_str.trim().to_lowercase();
            if trimmed != "y" && trimmed != "yes" && !trimmed.is_empty() {
                println!("[>] quitting.");
                std::process::exit(0);
            }
        }
    }

    if let Some(ref files) = args.files {
        for f in files {
            if !f.exists() {
                return Some(format!("\"{}\" does not exist", f.display()));
            }
        }
    }

    if let Some(ref m) = args.minimum {
        if m.chars().any(|c| !c.is_ascii_digit() && c != '.') {
            return Some(format!("invalid OS version: {}", m));
        }
    }

    if let Some(ref k) = args.icon {
        if !k.is_file() {
            return Some(format!("{} does not exist", k.display()));
        }
    }

    if let Some(ref l) = args.merge_plist {
        if !l.is_file() {
            return Some(format!("{} does not exist", l.display()));
        }
    }

    if let Some(ref cyan_files) = args.cyan {
        for cyan in cyan_files {
            if !cyan.is_file() {
                return Some(format!("{} does not exist", cyan.display()));
            }
        }
    }

    if let Some(ref x) = args.entitlements {
        if !x.is_file() {
            return Some(format!("{} does not exist", x.display()));
        }
        if plist::from_file::<_, plist::Value>(x).is_err() {
            return Some("couldn't parse given entitlements file".to_string());
        }
    }

    None
}
