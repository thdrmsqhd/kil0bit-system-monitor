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
                let temperature_c = fields.next().and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0);
                return Some(GpuReading { usage_percent, temperature_c });
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
