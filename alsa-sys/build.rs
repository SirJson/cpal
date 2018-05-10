extern crate pkg_config;

use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/*
* A big thanks to the rust-openssl Project!
*
* They are using a more complex build script that helped me understand how to
* integrate optional LIB and INCLUDE directories without introducing new dependencies to cpal.
*/

/// This script supports setting the directories for alsa manually. 
/// This is especially useful for cross-compiling since we often don't have pkg-config there
/// 
/// The following enviroment variables must be set if you don't or cant't use pkg-config
/// 
/// ALSA_LIB_DIR => The path to your custom alsa librarie(s)
/// ALSA_INCLUDE_DIR => The path to your custom alsa includes
/// 
/// If you want you can also set ALSA_LIB to control which library or libraries will linked instead of 'asound'
/// In the case you have more than one library you should put the ':' charcacter between each library
/// e.g: ALSA_LIBS = asound:asound_ext:asound_unicorn
/// 
/// This script also tries to figure out if we can dynamic link or not. If you insinst on static link alsa set ALSA_STATIC

fn env(name: &str) -> Option<OsString> {
    let prefix = env::var("TARGET").unwrap().to_uppercase().replace("-", "_");
    let prefixed = format!("{}_{}", prefix, name);
    println!("cargo:rerun-if-env-changed={}", prefixed);

    if let Some(var) = env::var_os(&prefixed) {
        return Some(var);
    }

    println!("cargo:rerun-if-env-changed={}", name);
    env::var_os(name)
}

fn main() {
    let lib_opt = env("ALSA_LIB_DIR").map(PathBuf::from);
    let include_opt = env("ALSA_INCLUDE_DIR").map(PathBuf::from);

    let (lib_dir, include_dir) = match (lib_opt, include_opt) {
        (lib_opt.is_some(), include_opt.is_none()) => panic!("ALSA_LIB_DIR is set but ALSA_INCLUDE_DIR IS not set.\nPlease set both enviroment variables if you want to set the library path without pkgcfg"),
        (lib_opt.is_none(), include_opt.is_some()) => panic!("ALSA_INCLUDE_DIR is set but ALSA_LIB_DIR IS not set.\nPlease set both enviroment variables if you want to set the library path without pkgcfg"),
        (lib_opt.is_some(), include_opt.is_some()) => (lib_opt.unwrap(), include_opt.unwrap()) // Success!
        (lib_opt.is_some(), include_opt.is_some()) => { pkg_config::find_library("alsa").unwrap(); return; }
    }

    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.to_string_lossy()
    );
    println!("cargo:include={}", include_dir.to_string_lossy());

    let libs_env = env("ALSA_LIBS");
    let libs = match libs_env.as_ref().and_then(|s| s.to_str()) {
        Some(ref v) => v.split(":").collect(),
        _ => vec!["asound"],
    };

    let kind = determine_mode(Path::new(&lib_dir), &libs);
    for lib in libs.into_iter() {
        println!("cargo:rustc-link-lib={}={}", kind, lib);
    }
}

/// Figure out if we should link ALSA static or dynamic
/// Pretty much the function that openssl-rust is using so we might have some sort of Mac support?
fn determine_mode(libdir: &Path, libs: &[&str]) -> &'static str {
    let kind = env("ALSA_STATIC");
    match kind.as_ref().and_then(|s| s.to_str()).map(|s| &s[..]) {
        Some("0") => return "dylib",
        Some(_) => return "static",
        None => {},
    }

    let files = libdir
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap())
        .map(|e| e.file_name())
        .filter_map(|e| e.into_string().ok())
        .collect::<HashSet<_>>();
    let can_static = libs.iter()
        .all(|l| files.contains(&format!("lib{}.a", l)) || files.contains(&format!("{}.lib", l)));
    let can_dylib = libs.iter().all(|l| {
        files.contains(&format!("lib{}.so", l)) || files.contains(&format!("{}.dll", l))
            || files.contains(&format!("lib{}.dylib", l))
    });
    match (can_static, can_dylib) {
        (true, false) => return "static",
        (false, true) => return "dylib",
        (false, false) => {
            panic!(
                "ALSA libdir at `{}` does not contain the required files to either statically or \
                 dynamically link ALSA",
                libdir.display()
            );
        },
        (true, true) => {},
    }

    "dylib"
}
