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

/// Precompute the Hann window once (it never changes for a fixed FFT size).
fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n as f32 - 1.0)).cos())
        .collect()
}

/// Turn the latest mono samples into five perceptual band levels (0–1).
/// `re`/`im` are scratch buffers reused across calls so the per-frame analysis
/// allocates nothing; `window` is the precomputed Hann window.
fn analyze(
    ring: &[f32],
    sample_rate: f32,
    window: &[f32],
    re: &mut [f32],
    im: &mut [f32],
) -> [f32; BANDS] {
    let n = ring.len();
    for i in 0..n {
        re[i] = ring[i] * window[i];
        im[i] = 0.0;
    }
    fft(re, im);

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
    /// Only the island shows the equalizer; targeting it avoids serializing the
    /// 30/sec spectrum payload into the settings and assistant webviews too.
    const VIZ_TARGET: &str = "island";
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
            // Allocate the FFT scratch + Hann window once, not per frame.
            let window = super::hann_window(FFT_SIZE);
            let mut re = vec![0f32; FFT_SIZE];
            let mut im = vec![0f32; FFT_SIZE];
            // When the mix is silent we stop spinning the FFT at 45fps: emit one
            // final zero frame, then idle on a long sleep until audio returns.
            let mut quiet_frames = 0u32;
            let mut sent_zero = false;
            // Last spectrum actually sent to the UI. We only push a new frame
            // when a band moved enough to be visible — every emit forces the
            // transparent island to recomposite over the (live) wallpaper, which
            // is the whole playing-vs-paused CPU gap.
            let mut last_sent = [0f32; BANDS];

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
                    analyze(&ring, sample_rate, &window, &mut re, &mut im)
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

                // Treat a fully-decayed spectrum as silence.
                let quiet = smooth.iter().all(|&v| v <= 0.001);
                if quiet {
                    quiet_frames = quiet_frames.saturating_add(1);
                } else {
                    quiet_frames = 0;
                    sent_zero = false;
                }

                // Once silent for ~0.6s, emit a single zero frame and then go
                // idle: skip the emit and sleep long so the FFT/IPC stop
                // burning CPU until sound returns.
                let idle = quiet_frames > 30;
                // Delta-gate: skip the frame entirely unless a band changed
                // visibly (~1px on a 16px bar). Sustained tones barely move the
                // bars, so most frames are skipped and the island stays as quiet
                // as when paused; transients still come through instantly.
                let moved = smooth
                    .iter()
                    .zip(last_sent.iter())
                    .any(|(a, b)| (a - b).abs() > 0.045);

                let emit = if idle { !sent_zero } else { moved };
                if emit {
                    if app.emit_to(VIZ_TARGET, "viz:levels", smooth.to_vec()).is_err() {
                        break;
                    }
                    last_sent = smooth;
                    if idle {
                        sent_zero = true;
                    }
                }

                // Poll the FFT at ~18fps while active (the delta-gate drops most
                // of these before they ever reach the UI); back off hard on
                // silence. The sleep is the FFT/analysis cadence, not the UI rate.
                std::thread::sleep(std::time::Duration::from_millis(if idle {
                    200
                } else {
                    55
                }));
            }
            let _ = client.Stop();
            Ok(())
        }
    }
}
