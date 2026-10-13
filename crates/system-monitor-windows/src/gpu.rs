//! GPU enumeration and optional vendor samples.
//! A missing utility or LUID is an unavailable reading, never a reason to stop.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Other,
    Unknown,
}

impl GpuVendor {
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro") || lower.contains("rtx") || lower.contains("gtx") {
            Self::Nvidia
        } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("rx ") {
            Self::Amd
        } else if lower == "default" || lower.is_empty() {
            Self::Unknown
        } else {
            Self::Other
        }
    }
}

/// Stable description of one adapter for Settings and provider selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuDevice {
    pub name: String,
    pub vendor: GpuVendor,
    pub index: u32,
    /// Lowercase `0x........_0x........` extracted from PDH GPU Engine paths when known.
    pub luid: Option<String>,
}

/// Resolved selection used by the telemetry sampler for one poll cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuSelection {
    pub vendor: GpuVendor,
    pub index: u32,
    pub luid: Option<String>,
    pub display_name: String,
}

pub struct GpuReading {
    pub usage_percent: f32,
    pub temperature_c: Option<f32>,
}

/// Parse `luid_0x...._0x....` from a PDH GPU Engine instance path.
pub fn parse_luid_from_instance(instance: &str) -> Option<String> {
    let lower = instance.to_ascii_lowercase();
    let rest = lower.split("luid_").nth(1)?;
    let mut parts = rest.split('_');
    let low = parts.next()?.trim();
    let high = parts.next()?.trim();
    if low.is_empty() || high.is_empty() {
        return None;
    }
    Some(format!("{low}_{high}"))
}

/// Parse `phys_N` from a PDH GPU Engine instance path.
pub fn parse_phys_index_from_instance(instance: &str) -> Option<u32> {
    let lower = instance.to_ascii_lowercase();
    let rest = lower.split("_phys_").nth(1)?;
    let token = rest.split('_').next()?.trim();
    token.parse().ok()
}

fn nvidia_names() -> Vec<String> {
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name", "--format=csv,noheader"]);
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let Ok(mut child) = command.spawn() else {
        return Vec::new();
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let Ok(output) = child.wait_with_output() else {
                    return Vec::new();
                };
                if !status.success() {
                    return Vec::new();
                }
                let Ok(text) = String::from_utf8(output.stdout) else {
                    return Vec::new();
                };
                return text
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
}

/// Enumerate adapters for Settings. Names prefer nvidia-smi when present;
/// LUIDs are filled from PDH GPU Engine instance paths when provided.
pub fn enumerate_gpus(engine_instances: &[String]) -> Vec<GpuDevice> {
    let mut devices = Vec::new();
    let nvidia = nvidia_names();
    for (index, name) in nvidia.into_iter().enumerate() {
        let luid = engine_instances.iter().find_map(|instance| {
            let phys = parse_phys_index_from_instance(instance)?;
            if phys == index as u32 {
                parse_luid_from_instance(instance)
            } else {
                None
            }
        });
        devices.push(GpuDevice {
            name,
            vendor: GpuVendor::Nvidia,
            index: index as u32,
            luid,
        });
    }

    // Collect unique phys indices from engine paths not already covered.
    let mut seen_phys = std::collections::BTreeSet::new();
    for device in &devices {
        seen_phys.insert(device.index);
    }
    for instance in engine_instances {
        let Some(phys) = parse_phys_index_from_instance(instance) else {
            continue;
        };
        if !seen_phys.insert(phys) {
            continue;
        }
        let luid = parse_luid_from_instance(instance);
        devices.push(GpuDevice {
            name: format!("GPU {phys}"),
            vendor: GpuVendor::Unknown,
            index: phys,
            luid,
        });
    }
    devices.sort_by_key(|device| device.index);
    devices
}

/// Map Settings `gpu_adapter` + `gpu_index` to a concrete selection.
pub fn resolve_selection(
    adapter: &str,
    index: u32,
    devices: &[GpuDevice],
) -> GpuSelection {
    let vendor = GpuVendor::from_name(adapter);
    if adapter.eq_ignore_ascii_case("Default") || adapter.is_empty() {
        if let Some(first) = devices.first() {
            return GpuSelection {
                vendor: first.vendor,
                index: first.index,
                luid: first.luid.clone(),
                display_name: first.name.clone(),
            };
        }
        return GpuSelection {
            vendor: GpuVendor::Unknown,
            index,
            luid: None,
            display_name: "Default".into(),
        };
    }

    if let Some(matched) = devices.iter().find(|device| device.name.eq_ignore_ascii_case(adapter)) {
        return GpuSelection {
            vendor: matched.vendor,
            index: matched.index,
            luid: matched.luid.clone(),
            display_name: matched.name.clone(),
        };
    }

    if let Some(matched) = devices.iter().find(|device| {
        device.index == index
            && (vendor == GpuVendor::Unknown
                || vendor == device.vendor
                || device.vendor == GpuVendor::Unknown)
    }) {
        return GpuSelection {
            vendor: if vendor == GpuVendor::Unknown {
                matched.vendor
            } else {
                vendor
            },
            index: matched.index,
            luid: matched.luid.clone(),
            display_name: matched.name.clone(),
        };
    }

    GpuSelection {
        vendor,
        index,
        luid: None,
        display_name: adapter.to_owned(),
    }
}

pub fn sample_nvidia(index: usize) -> Option<GpuReading> {
    let mut command = Command::new("nvidia-smi");
    command.args([
        "--query-gpu=utilization.gpu,temperature.gpu",
        "--format=csv,noheader,nounits",
    ]);
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().ok()?;
                if !status.success() {
                    return None;
                }
                let text = String::from_utf8(output.stdout).ok()?;
                let line = text.lines().nth(index)?;
                let mut fields = line.split(',').map(str::trim);
                let usage_percent = fields.next()?.parse::<f32>().ok()?;
                if !usage_percent.is_finite() || !(0.0..=100.0).contains(&usage_percent) {
                    return None;
                }
                let temperature_c = fields
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0);
                return Some(GpuReading {
                    usage_percent,
                    temperature_c,
                });
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(windows)]
pub fn sample_amd(index: usize) -> Option<GpuReading> {
    use std::ffi::{c_char, c_void};
    use std::mem::{size_of, transmute};
    use std::ptr::null_mut;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Sensor {
        supported: i32,
        value: i32,
    }
    #[repr(C)]
    struct PmLog {
        size: i32,
        sensors: [Sensor; 256],
    }
    type Create = unsafe extern "C" fn(extern "C" fn(i32) -> *mut c_void, i32, *mut *mut c_void) -> i32;
    type Destroy = unsafe extern "C" fn(*mut c_void) -> i32;
    type Count = unsafe extern "C" fn(*mut c_void, *mut i32) -> i32;
    type Query = unsafe extern "C" fn(*mut c_void, i32, *mut PmLog) -> i32;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn FreeLibrary(library: *mut c_void) -> i32;
        fn GetProcAddress(library: *mut c_void, name: *const c_char) -> *mut c_void;
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    }
    extern "C" fn allocate(size: i32) -> *mut c_void {
        if size <= 0 {
            return null_mut();
        }
        unsafe { GlobalAlloc(0, size as usize) }
    }
    unsafe fn symbol(library: *mut c_void, name: &'static [u8]) -> Option<*mut c_void> {
        let ptr = GetProcAddress(library, name.as_ptr().cast());
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    let library_name: Vec<u16> = "atiadlxx.dll\0".encode_utf16().collect();
    unsafe {
        let library = LoadLibraryW(library_name.as_ptr());
        if library.is_null() {
            return None;
        }
        let result = (|| {
            let create: Create = transmute(symbol(library, b"ADL2_Main_Control_Create\0")?);
            let destroy: Destroy = transmute(symbol(library, b"ADL2_Main_Control_Destroy\0")?);
            let count: Count = transmute(symbol(library, b"ADL2_Adapter_NumberOfAdapters_Get\0")?);
            let query: Query = transmute(symbol(library, b"ADL2_New_QueryPMLogData_Get\0")?);
            let mut context = null_mut();
            if create(allocate, 1, &mut context) != 0 || context.is_null() {
                return None;
            }
            let reading = (|| {
                let mut adapters = 0;
                if count(context, &mut adapters) != 0 || index >= adapters.max(0) as usize {
                    return None;
                }
                let mut data = PmLog {
                    size: size_of::<PmLog>() as i32,
                    sensors: [Sensor {
                        supported: 0,
                        value: 0,
                    }; 256],
                };
                if query(context, index as i32, &mut data) != 0 {
                    return None;
                }
                let usage = data.sensors[19];
                let usage_percent = if usage.supported == 1 && (0..=100).contains(&usage.value) {
                    usage.value as f32
                } else {
                    return None;
                };
                let temperature_c = [28, 8, 27]
                    .into_iter()
                    .map(|sensor| data.sensors[sensor])
                    .find(|sensor| sensor.supported == 1 && (1..120).contains(&sensor.value))
                    .map(|sensor| sensor.value as f32);
                Some(GpuReading {
                    usage_percent,
                    temperature_c,
                })
            })();
            destroy(context);
            reading
        })();
        FreeLibrary(library);
        result
    }
}

#[cfg(not(windows))]
pub fn sample_amd(_: usize) -> Option<GpuReading> {
    None
}

#[cfg(windows)]
pub fn sample_d3dkmt_temperature(engine_instance: &str) -> Option<f32> {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    #[repr(C)]
    struct Luid {
        low: u32,
        high: i32,
    }
    #[repr(C)]
    struct Open {
        luid: Luid,
        adapter: u32,
    }
    #[repr(C)]
    struct Query {
        adapter: u32,
        kind: u32,
        data: *mut c_void,
        size: u32,
    }
    #[repr(C)]
    struct PerfData {
        thermal_throttling: u32,
        current_frequency: u64,
        max_frequency: u64,
        max_frequency_oc: u64,
        memory_frequency: u64,
        memory_frequency_oc: u64,
        fan_speed: u32,
        temperature: u32,
        voltage: u32,
        memory_usage: u32,
        max_memory_usage: u32,
        core_clock: u64,
        memory_clock: u64,
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn D3DKMTOpenAdapterFromLuid(data: *mut Open) -> i32;
        fn D3DKMTQueryAdapterInfo(data: *mut Query) -> i32;
        fn D3DKMTCloseAdapter(data: *mut u32) -> i32;
    }
    let luid = parse_luid_from_instance(engine_instance)?;
    let mut parts = luid.split('_');
    let low = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    let high = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()? as i32;
    let mut open = Open {
        luid: Luid { low, high },
        adapter: 0,
    };
    unsafe {
        if D3DKMTOpenAdapterFromLuid(&mut open) != 0 {
            return None;
        }
        let mut data: PerfData = zeroed();
        let mut query = Query {
            adapter: open.adapter,
            kind: 35,
            data: (&mut data as *mut PerfData).cast(),
            size: size_of::<PerfData>() as u32,
        };
        let status = D3DKMTQueryAdapterInfo(&mut query);
        D3DKMTCloseAdapter(&mut open.adapter);
        if status == 0 && (1..1200).contains(&data.temperature) {
            Some(data.temperature as f32 / 10.0)
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn sample_d3dkmt_temperature(_: &str) -> Option<f32> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        enumerate_gpus, parse_luid_from_instance, parse_phys_index_from_instance, resolve_selection,
        GpuVendor,
    };

    #[test]
    fn parses_luid_and_phys_from_engine_path() {
        let path = r"\\GPU Engine(luid_0x00000000_0x0000e3aa_phys_0_eng_0_engtype_3D)\\Utilization Percentage";
        assert_eq!(
            parse_luid_from_instance(path).as_deref(),
            Some("0x00000000_0x0000e3aa")
        );
        assert_eq!(parse_phys_index_from_instance(path), Some(0));
    }

    #[test]
    fn enumerate_fills_placeholder_devices_from_phys_indices() {
        let instances = vec![
            r"\\GPU Engine(luid_0x00000000_0x0000e3aa_phys_0_eng_0_engtype_3D)\\Utilization Percentage"
                .into(),
            r"\\GPU Engine(luid_0x00000000_0x0000abcd_phys_1_eng_0_engtype_3D)\\Utilization Percentage"
                .into(),
        ];
        let devices = enumerate_gpus(&instances);
        assert!(devices.iter().any(|d| d.index == 0));
        assert!(devices.iter().any(|d| d.index == 1));
        let zero = devices.iter().find(|d| d.index == 0).unwrap();
        assert_eq!(zero.luid.as_deref(), Some("0x00000000_0x0000e3aa"));
    }

    #[test]
    fn resolve_prefers_exact_name_then_index() {
        let devices = vec![super::GpuDevice {
            name: "NVIDIA GeForce RTX 4070".into(),
            vendor: GpuVendor::Nvidia,
            index: 0,
            luid: Some("0x00000000_0x0000e3aa".into()),
        }];
        let by_name = resolve_selection("NVIDIA GeForce RTX 4070", 9, &devices);
        assert_eq!(by_name.index, 0);
        assert_eq!(by_name.luid.as_deref(), Some("0x00000000_0x0000e3aa"));
        let by_vendor = resolve_selection("NVIDIA", 0, &devices);
        assert_eq!(by_vendor.vendor, GpuVendor::Nvidia);
        assert_eq!(by_vendor.luid.as_deref(), Some("0x00000000_0x0000e3aa"));
    }
}
