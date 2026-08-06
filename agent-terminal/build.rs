#[cfg(windows)]
mod build_support;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support.rs");

    #[cfg(windows)]
    windows::build().expect("failed to build Lucidity Agent Windows resources");
}

#[cfg(windows)]
mod windows {
    use super::build_support::{
        profile_output_dir, provision_if_stale, resolve_build_version, BuildVersionResolution,
        ContentFingerprint, BUILD_VERSION_ENV,
    };
    use anyhow::{Context, Result};
    use std::fmt::Write as _;
    use std::fs;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    const RUNTIME_FILES: &[(&str, &str)] = &[
        ("assets/windows/conhost/conpty.dll", "conpty.dll"),
        ("assets/windows/conhost/OpenConsole.exe", "OpenConsole.exe"),
        ("assets/windows/angle/libEGL.dll", "libEGL.dll"),
        ("assets/windows/angle/libGLESv2.dll", "libGLESv2.dll"),
        ("assets/windows/mesa/opengl32.dll", "mesa/opengl32.dll"),
    ];

    pub fn build() -> Result<()> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_dir = manifest_dir
            .parent()
            .context("agent-terminal must be a root-sibling package")?;
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").context("OUT_DIR is not set")?);
        let profile_dir = profile_output_dir(&out_dir)?;

        provision_runtime_files(repo_dir, &profile_dir)?;
        compile_resources(&manifest_dir, &out_dir)?;
        Ok(())
    }

    fn provision_runtime_files(repo_dir: &Path, profile_dir: &Path) -> Result<()> {
        let mut receipt = String::from("format=lucidity-runtime-files-v1\n");

        for (source_relative, destination_relative) in RUNTIME_FILES {
            let source = repo_dir.join(source_relative);
            let destination = profile_dir.join(destination_relative);
            println!("cargo:rerun-if-changed={}", source.display());
            println!("cargo:rerun-if-changed={}", destination.display());

            let outcome = provision_if_stale(&source, &destination).with_context(|| {
                format!(
                    "provision runtime file {} -> {}",
                    source.display(),
                    destination.display()
                )
            })?;
            let ContentFingerprint { byte_len, sha256 } = outcome.source;
            writeln!(
                receipt,
                "{}\t{}\t{}\t{}",
                if outcome.copied { "copied" } else { "current" },
                destination_relative.replace('\\', "/"),
                byte_len,
                sha256_hex(&sha256)
            )?;
        }

        fs::create_dir_all(profile_dir)?;
        fs::write(profile_dir.join("lucidity-runtime-files.txt"), receipt)?;
        Ok(())
    }

    fn sha256_hex(digest: &[u8; 32]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn compile_resources(manifest_dir: &Path, out_dir: &Path) -> Result<()> {
        let windows_dir = manifest_dir.join("assets").join("windows");
        let manifest = windows_dir.join("lucidity.manifest");
        let icon = windows_dir.join("weld-frame.ico");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rerun-if-changed={}", icon.display());

        println!("cargo:rerun-if-env-changed={BUILD_VERSION_ENV}");
        let BuildVersionResolution {
            version,
            source: _,
            tag_path,
            git_watch_paths,
        } = resolve_build_version(
            manifest_dir
                .parent()
                .context("agent-terminal must be a root-sibling package")?,
        )?;
        println!("cargo:rerun-if-changed={}", tag_path.display());
        if let Some(paths) = git_watch_paths {
            println!("cargo:rerun-if-changed={}", paths.head.display());
            if let Some(symbolic_ref) = paths.symbolic_ref {
                println!("cargo:rerun-if-changed={}", symbolic_ref.display());
            }
            println!("cargo:rerun-if-changed={}", paths.packed_refs.display());
        }
        let resource_path = out_dir.join("lucidity-agent.rc");
        let mut resource = fs::File::create(&resource_path)?;
        write!(
            resource,
            r#"#include <winres.h>
#define IDI_ICON 0x101
1 RT_MANIFEST "{manifest}"
IDI_ICON ICON "{icon}"
VS_VERSION_INFO VERSIONINFO
FILEVERSION     0,1,0,0
PRODUCTVERSION  0,1,0,0
FILEFLAGSMASK   VS_FFI_FILEFLAGSMASK
FILEFLAGS       0
FILEOS          VOS__WINDOWS32
FILETYPE        VFT_APP
FILESUBTYPE     VFT2_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904E4"
        BEGIN
            VALUE "CompanyName",      "Lucidity Contributors\0"
            VALUE "FileDescription",  "Lucidity Agent Terminal\0"
            VALUE "FileVersion",      "{version}\0"
            VALUE "InternalName",     "agent\0"
            VALUE "LegalCopyright",   "Lucidity contributors, MIT licensed\0"
            VALUE "OriginalFilename", "agent.exe\0"
            VALUE "ProductName",      "Lucidity\0"
            VALUE "ProductVersion",   "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1252
    END
END
"#,
            manifest = rc_path(&manifest),
            icon = rc_path(&icon),
        )?;
        drop(resource);

        let target = std::env::var("TARGET").context("TARGET is not set")?;
        if let Some(tool) = cc::windows_registry::find_tool(&target, "cl.exe") {
            for (key, value) in tool.env() {
                std::env::set_var(key, value);
            }
        }
        embed_resource::compile(&resource_path);
        Ok(())
    }

    fn rc_path(path: &Path) -> String {
        path.display().to_string().replace('\\', "\\\\")
    }
}
