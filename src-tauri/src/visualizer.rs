//! Real-time audio spectrum for the island equalizer.
//!
//! Captures the system's output mix via WASAPI loopback, runs a small FFT and
//! emits five log-spaced frequency-band levels (0.0–1.0) to the island webview
//! as `viz:levels`, so the equalizer bars actually move with the music instead
//! of running a canned animation. When nothing is playing the bands decay to 0.

use tauri::AppHandle;

/// FFT window size (power of two). ~21ms at 48kHz — snappy but stable.
const FFT_SIZE: usize = 1024;
/// Number of bars in the UI equalizer.
const BANDS: usize = 5;

/// Spawn the capture + analysis thread. No-op off Windows.
pub fn spawn(app: AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || loop {
        // If the device is lost (default endpoint changed, etc.) the capture
        // returns; wait a moment and re-open it.
        let _ = win::run(&app);
        std::thread::sleep(std::time::Duration::from_millis(800));
    });
    #[cfg(not(windows))]
    let _ = app;
}

/// In-place iterative radix-2 Cooley–Tukey FFT.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = i + k;
                let b = a + len / 2;
                let tr = cr * re[b] - ci * im[b];
                let ti = cr * im[b] + ci * re[b];
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Turn the latest mono samples into five perceptual band levels (0–1).
fn analyze(ring: &[f32], sample_rate: f32) -> [f32; BANDS] {
    let n = ring.len();
    let mut re = vec![0f32; n];
    let mut im = vec![0f32; n];
    // Hann window to curb spectral leakage.
    for i in 0..n {
        let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n as f32 - 1.0)).cos();
        re[i] = ring[i] * w;
    }
    fft(&mut re, &mut im);

    // Log-spaced band edges (Hz) + a gentle treble lift (highs read quieter).
    const EDGES: [f32; BANDS + 1] = [30.0, 120.0, 350.0, 1000.0, 3000.0, 12000.0];
    const BOOST: [f32; BANDS] = [1.0, 1.1, 1.35, 1.7, 2.2];
    let bin_hz = sample_rate / n as f32;
    let mut out = [0f32; BANDS];
    for b in 0..BANDS {
        let lo = ((EDGES[b] / bin_hz).floor() as usize).max(1);
        let hi = ((EDGES[b + 1] / bin_hz).ceil() as usize).min(n / 2);
        let mut power = 0.0f32;
        let mut count = 0u32;
        let mut k = lo;
        while k < hi {
            let m = (re[k] * re[k] + im[k] * im[k]).sqrt() / n as f32;
            power += m * m;
            count += 1;
            k += 1;
        }
        let amp = if count > 0 { (power / count as f32).sqrt() } else { 0.0 } * BOOST[b];
        // Amplitude → dB → normalized [floor, ceil] dB window.
        let db = 20.0 * (amp + 1e-7).log10();
        out[b] = ((db + 68.0) / 53.0).clamp(0.0, 1.0);
    }
    out
}

#[cfg(windows)]
mod win {
    use super::{analyze, AppHandle, BANDS, FFT_SIZE};
    use tauri::Emitter;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_LOOPBACK,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    pub fn run(app: &AppHandle) -> windows::core::Result<()> {
        crate::media::ensure_com();
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            let wfx = client.GetMixFormat()?;
            let channels = (*wfx).nChannels as usize;
            let sample_rate = (*wfx).nSamplesPerSec as f32;
            let bits = (*wfx).wBitsPerSample;

            // 100ms buffer, shared loopback mode.
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                1_000_000,
                0,
                wfx,
                None,
            )?;
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;

            let mut ring = vec![0f32; FFT_SIZE];
            let mut smooth = [0f32; BANDS];

            loop {
                let mut got = 0usize;
                loop {
                    let packet = capture.GetNextPacketSize()?;
                    if packet == 0 {
                        break;
                    }
                    let mut data: *mut u8 = std::ptr::null_mut();
                    let mut frames = 0u32;
                    let mut flags = 0u32;
                    capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                    let frames = frames as usize;
                    let silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;

                    if silent || data.is_null() || bits != 32 {
                        ring.extend(std::iter::repeat(0.0).take(frames));
                    } else {
                        let floats = std::slice::from_raw_parts(
                            data as *const f32,
                            frames * channels,
                        );
                        for f in 0..frames {
                            let mut s = 0.0f32;
                            for c in 0..channels {
                                s += floats[f * channels + c];
                            }
                            ring.push(s / channels as f32);
                        }
                    }
                    got += frames;
                    capture.ReleaseBuffer(frames as u32)?;
                }

                // Keep only the most recent FFT_SIZE samples.
                if ring.len() > FFT_SIZE {
                    ring.drain(0..ring.len() - FFT_SIZE);
                }

                let raw = if got > 0 {
                    analyze(&ring, sample_rate)
                } else {
                    [0f32; BANDS]
                };

                // Fast attack, slow decay so bars pop on transients then settle.
                for i in 0..BANDS {
                    smooth[i] = if raw[i] > smooth[i] {
                        raw[i]
                    } else {
                        smooth[i] * 0.80 + raw[i] * 0.20
                    };
                }

                if app.emit("viz:levels", smooth.to_vec()).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(22));
            }
            let _ = client.Stop();
            Ok(())
        }
    }
}
