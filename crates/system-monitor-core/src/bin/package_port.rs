use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = package() {
        eprintln!("package-port: {error}");
        std::process::exit(1);
    }
}

fn package() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let executable = PathBuf::from(
        args.next()
            .ok_or("usage: package-port <system-monitor-windows.exe> <output-dir>")?,
    );
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: package-port <system-monitor-windows.exe> <output-dir>")?,
    );
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    package_name_is_rust(&executable).map_err(std::io::Error::other)?;
    if !executable.is_file() {
        return Err("Rust release executable does not exist".into());
    }
    if output.exists() {
        return Err("output directory already exists".into());
    }
    fs::create_dir_all(&output)?;
    fs::copy(&executable, output.join("Kil0bitSystemMonitorRust.exe"))?;
    copy_if_present(Path::new("LICENSE"), &output)?;
    fs::write(output.join("README.txt"), concat!(
        "Kil0bit System Monitor Rust port\r\n\r\n",
        "This portable build is a native Rust/Win32 application. It does not include the original .NET executable or a managed runtime.\r\n",
        "Settings are stored under %APPDATA%\\Kil0bitSystemMonitorRust. API credentials are stored separately with Windows DPAPI.\r\n",
        "Launch this executable to show Settings; launch with --startup to suppress the first-run Settings window.\r\n",
        "Original project: kil0bit-kb/kil0bit-system-monitor (MIT). The included LICENSE applies to this derivative.\r\n",
        "For a portable upgrade, close the monitor and replace only the executable; settings remain in AppData.\r\n",
        "To remove it, disable Launch on Startup in Settings, close the monitor, and delete the portable folder. Optionally remove the Rust-specific AppData and LocalAppData folders.\r\n",
        "Hardware-specific GPU paths and Windows 11 interactions require device-side validation. The original executable was not run for comparison.\r\n",
    ))?;
    Ok(())
}

fn copy_if_present(source: &Path, output: &Path) -> Result<(), std::io::Error> {
    if source.is_file() {
        fs::copy(source, output.join("LICENSE"))?;
    }
    Ok(())
}

fn package_name_is_rust(path: &Path) -> Result<(), &'static str> {
    if path.file_name().and_then(|name| name.to_str()) == Some("system-monitor-windows.exe") {
        Ok(())
    } else {
        Err("not the Rust executable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_rust_executable_names() {
        assert!(package_name_is_rust(Path::new("Kil0bitSystemMonitor.exe")).is_err());
        assert!(package_name_is_rust(Path::new("system-monitor-windows.exe")).is_ok());
    }
}
