//! Windows-user DPAPI storage for provider credentials. Secrets are never part of JSON settings.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct SecretStore {
    path: PathBuf,
}

impl SecretStore {
    pub fn opencode() -> io::Result<Self> {
        let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable")
        })?;
        Ok(Self {
            path: PathBuf::from(local)
                .join("Kil0bitSystemMonitorRust")
                .join("secrets")
                .join("opencode.dpapi"),
        })
    }

    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn save(&self, key: &str) -> io::Result<()> {
        if key.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "API key is empty",
            ));
        }
        let plaintext = Zeroizing::new(key.as_bytes().to_vec());
        let encrypted = protect(&plaintext)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = self.path.with_extension("dpapi.tmp");
        fs::write(&temp, encrypted)?;
        replace(&temp, &self.path)
    }

    pub fn load(&self) -> io::Result<Zeroizing<String>> {
        let encrypted = fs::read(&self.path)?;
        let plaintext = Zeroizing::new(unprotect(&encrypted)?);
        let key = String::from_utf8(plaintext.to_vec()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "stored credential is invalid")
        })?;
        Ok(Zeroizing::new(key))
    }

    pub fn remove(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }
}

#[cfg(windows)]
fn protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            input: *const DataBlob,
            description: *const u16,
            entropy: *const DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt: *const std::ffi::c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let input = DataBlob {
        size: plaintext.len() as u32,
        data: plaintext.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        size: 0,
        data: std::ptr::null_mut(),
    };
    if unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            1,
            &mut output,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.data, output.size as usize) }.to_vec();
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn unprotect(ciphertext: &[u8]) -> io::Result<Vec<u8>> {
    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptUnprotectData(
            input: *const DataBlob,
            description: *mut *mut u16,
            entropy: *const DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt: *const std::ffi::c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let input = DataBlob {
        size: ciphertext.len() as u32,
        data: ciphertext.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        size: 0,
        data: std::ptr::null_mut(),
    };
    if unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            1,
            &mut output,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.data, output.size as usize) }.to_vec();
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(bytes)
}

#[cfg(not(windows))]
fn protect(_: &[u8]) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "DPAPI secret storage requires Windows",
    ))
}

#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "DPAPI secret storage requires Windows",
    ))
}

#[cfg(windows)]
fn replace(temp: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn replace(temp: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temp, destination)
}

#[cfg(all(test, windows))]
mod tests {
    use super::SecretStore;

    #[test]
    fn dpapi_round_trip_stores_ciphertext_and_removes_it() {
        let path =
            std::env::temp_dir().join(format!("kil0bit-dpapi-test-{}.bin", std::process::id()));
        let store = SecretStore::from_path(path.clone());
        let fixture = "fixture-secret-not-a-real-key";
        store.save(fixture).unwrap();
        let ciphertext = std::fs::read(&path).unwrap();
        assert_ne!(ciphertext, fixture.as_bytes());
        let recovered = store.load().unwrap();
        assert_eq!(&*recovered, fixture);
        store.remove().unwrap();
        assert!(!store.exists());
    }
}
