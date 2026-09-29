//! Optional Windows performance counters for physical disks and GPU engines.
//! Counter failures yield no sample; the remaining metrics continue updating.

#[cfg(windows)]
mod native {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    type Handle = *mut c_void;
    const PDH_FMT_DOUBLE: u32 = 0x0000_0200;

    #[repr(C)]
    struct CounterValue {
        status: u32,
        value: f64,
    }

    #[link(name = "pdh")]
    extern "system" {
        fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut Handle) -> i32;
        fn PdhCloseQuery(query: Handle) -> i32;
        fn PdhExpandWildCardPathW(
            source: *const u16,
            path: *const u16,
            buffer: *mut u16,
            length: *mut u32,
            flags: u32,
        ) -> i32;
        fn PdhAddEnglishCounterW(
            query: Handle,
            path: *const u16,
            user: usize,
            counter: *mut Handle,
        ) -> i32;
        fn PdhCollectQueryData(query: Handle) -> i32;
        fn PdhGetFormattedCounterValue(
            counter: Handle,
            format: u32,
            type_out: *mut u32,
            value: *mut CounterValue,
        ) -> i32;
    }

    pub struct CounterGroup {
        query: Handle,
        counters: Vec<(String, Handle)>,
    }

    // PDH query ownership is transferred once to the telemetry worker and never shared.
    unsafe impl Send for CounterGroup {}

    impl CounterGroup {
        pub fn new(pattern: &str) -> Option<Self> {
            unsafe {
                let mut query = null_mut();
                if PdhOpenQueryW(null(), 0, &mut query) != 0 {
                    return None;
                }
                let path: Vec<u16> = pattern.encode_utf16().chain(Some(0)).collect();
                let mut length = 0_u32;
                PdhExpandWildCardPathW(null(), path.as_ptr(), null_mut(), &mut length, 0);
                if length == 0 || length > 131_072 {
                    PdhCloseQuery(query);
                    return None;
                }
                let mut buffer = vec![0_u16; length as usize];
                if PdhExpandWildCardPathW(
                    null(),
                    path.as_ptr(),
                    buffer.as_mut_ptr(),
                    &mut length,
                    0,
                ) != 0
                {
                    PdhCloseQuery(query);
                    return None;
                }
                let mut counters = Vec::new();
                for entry in buffer
                    .split(|ch| *ch == 0)
                    .filter(|entry| !entry.is_empty())
                {
                    let mut counter = null_mut();
                    if PdhAddEnglishCounterW(query, entry.as_ptr(), 0, &mut counter) == 0 {
                        counters.push((String::from_utf16_lossy(entry), counter));
                    }
                }
                if counters.is_empty() {
                    PdhCloseQuery(query);
                    return None;
                }
                PdhCollectQueryData(query);
                Some(Self { query, counters })
            }
        }

        pub fn sample(&self) -> Vec<(String, f32)> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return Vec::new();
                }
                self.counters
                    .iter()
                    .filter_map(|(name, counter)| {
                        let mut value = CounterValue {
                            status: 0,
                            value: 0.0,
                        };
                        if PdhGetFormattedCounterValue(
                            *counter,
                            PDH_FMT_DOUBLE,
                            null_mut(),
                            &mut value,
                        ) != 0
                            || value.status > 1
                            || !value.value.is_finite()
                        {
                            return None;
                        }
                        Some((name.clone(), value.value.clamp(0.0, 100.0) as f32))
                    })
                    .collect()
            }
        }
    }

    impl Drop for CounterGroup {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(windows)]
pub use native::CounterGroup;

#[cfg(not(windows))]
pub struct CounterGroup;

#[cfg(not(windows))]
impl CounterGroup {
    pub fn new(_: &str) -> Option<Self> {
        None
    }
    pub fn sample(&self) -> Vec<(String, f32)> {
        Vec::new()
    }
}
