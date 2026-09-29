//! Bounded, optional NVIDIA provider. A missing vendor utility is an unavailable
//! reading, never a reason for the system monitor to stop.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct GpuReading {
    pub usage_percent: f32,
    pub temperature_c: Option<f32>,
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
    struct Sensor { supported: i32, value: i32 }
    #[repr(C)]
    struct PmLog { size: i32, sensors: [Sensor; 256] }
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
        if size <= 0 { return null_mut(); }
        unsafe { GlobalAlloc(0, size as usize) }
    }
    unsafe fn symbol(library: *mut c_void, name: &'static [u8]) -> Option<*mut c_void> {
        let ptr = GetProcAddress(library, name.as_ptr().cast());
        if ptr.is_null() { None } else { Some(ptr) }
    }
    let library_name: Vec<u16> = "atiadlxx.dll\0".encode_utf16().collect();
    unsafe {
        let library = LoadLibraryW(library_name.as_ptr());
        if library.is_null() { return None; }
        let result = (|| {
            let create: Create = transmute(symbol(library, b"ADL2_Main_Control_Create\0")?);
            let destroy: Destroy = transmute(symbol(library, b"ADL2_Main_Control_Destroy\0")?);
            let count: Count = transmute(symbol(library, b"ADL2_Adapter_NumberOfAdapters_Get\0")?);
            let query: Query = transmute(symbol(library, b"ADL2_New_QueryPMLogData_Get\0")?);
            let mut context = null_mut();
            if create(allocate, 1, &mut context) != 0 || context.is_null() { return None; }
            let reading = (|| {
                let mut adapters = 0;
                if count(context, &mut adapters) != 0 || index >= adapters.max(0) as usize { return None; }
                let mut data = PmLog {
                    size: size_of::<PmLog>() as i32,
                    sensors: [Sensor { supported: 0, value: 0 }; 256],
                };
                if query(context, index as i32, &mut data) != 0 { return None; }
                let usage = data.sensors[19];
                let usage_percent = if usage.supported == 1 && (0..=100).contains(&usage.value) {
                    usage.value as f32
                } else { return None; };
                let temperature_c = [28, 8, 27].into_iter()
                    .map(|sensor| data.sensors[sensor])
                    .find(|sensor| sensor.supported == 1 && (1..120).contains(&sensor.value))
                    .map(|sensor| sensor.value as f32);
                Some(GpuReading { usage_percent, temperature_c })
            })();
            destroy(context);
            reading
        })();
        FreeLibrary(library);
        result
    }
}

#[cfg(not(windows))]
pub fn sample_amd(_: usize) -> Option<GpuReading> { None }

#[cfg(windows)]
pub fn sample_d3dkmt_temperature(engine_instance: &str) -> Option<f32> {
    use std::mem::{size_of, zeroed};
    use std::ffi::c_void;
    #[repr(C)]
    struct Luid { low: u32, high: i32 }
    #[repr(C)]
    struct Open { luid: Luid, adapter: u32 }
    #[repr(C)]
    struct Query { adapter: u32, kind: u32, data: *mut c_void, size: u32 }
    #[repr(C)]
    struct PerfData {
        thermal_throttling: u32,
        current_frequency: u64, max_frequency: u64, max_frequency_oc: u64,
        memory_frequency: u64, memory_frequency_oc: u64,
        fan_speed: u32, temperature: u32, voltage: u32,
        memory_usage: u32, max_memory_usage: u32,
        core_clock: u64, memory_clock: u64,
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn D3DKMTOpenAdapterFromLuid(data: *mut Open) -> i32;
        fn D3DKMTQueryAdapterInfo(data: *mut Query) -> i32;
        fn D3DKMTCloseAdapter(data: *mut u32) -> i32;
    }
    let lower = engine_instance.to_ascii_lowercase();
    let luid = lower.split("luid_").nth(1)?
        .split("_phys_").next()?;
    let mut parts = luid.split('_');
    let low = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    let high = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()? as i32;
    let mut open = Open { luid: Luid { low, high }, adapter: 0 };
    unsafe {
        if D3DKMTOpenAdapterFromLuid(&mut open) != 0 { return None; }
        let mut data: PerfData = zeroed();
        let mut query = Query {
            adapter: open.adapter, kind: 35,
            data: (&mut data as *mut PerfData).cast(), size: size_of::<PerfData>() as u32,
        };
        let status = D3DKMTQueryAdapterInfo(&mut query);
        D3DKMTCloseAdapter(&mut open.adapter);
        if status == 0 && (1..1200).contains(&data.temperature) {
            Some(data.temperature as f32 / 10.0)
        } else { None }
    }
}

#[cfg(not(windows))]
pub fn sample_d3dkmt_temperature(_: &str) -> Option<f32> { None }
