//! Native PulseAudio output.
//!
//! The ALSA path reaches PulseAudio through libasound's plugin, which adds a
//! format conversion and a client thread of its own. This opens a stream in
//! the sound server directly, through libpulse-simple, so a host that runs
//! PulseAudio can skip that layer.
//!
//! The library is loaded when the backend is asked for, not linked: a build
//! without it still starts, and ALSA is still the default.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use rodio::mixer::MixerSource;

/// Frames written to the server at a time.
const FRAMES: usize = 2048;
/// Channels the mixer feeding this stream produces.
pub const CHANNELS: u16 = 2;
/// Sample rate the mixer feeding this stream runs at.
pub const RATE: u32 = 48_000;
/// `PA_SAMPLE_FLOAT32NE`, which is what rodio mixes in.
#[cfg(target_endian = "little")]
const FLOAT32: c_int = 5;
/// `PA_SAMPLE_FLOAT32BE`, for the hosts where that is native.
#[cfg(target_endian = "big")]
const FLOAT32: c_int = 6;
/// `PA_STREAM_PLAYBACK`.
const PLAYBACK: c_int = 1;

/// The C `pa_sample_spec`.
#[repr(C)]
struct SampleSpec {
    /// `pa_sample_format_t`.
    format: c_int,
    /// Frames per second.
    rate: u32,
    /// Interleaved channels.
    channels: u8,
}

/// `pa_simple_new`.
type SimpleNew = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
    *const c_char,
    *const SampleSpec,
    *const c_void,
    *const c_void,
    *mut c_int,
) -> *mut c_void;
/// `pa_simple_write`.
type SimpleWrite = unsafe extern "C" fn(*mut c_void, *const c_void, usize, *mut c_int) -> c_int;
/// `pa_simple_free`.
type SimpleFree = unsafe extern "C" fn(*mut c_void);
/// `pa_strerror`.
type StrError = unsafe extern "C" fn(c_int) -> *const c_char;

/// A stream playing into the sound server.
pub struct Sink {
    /// Set when the sink is dropped, so the writing thread returns.
    stop: Arc<AtomicBool>,
    /// The thread pulling the mixer into the stream.
    thread: Option<JoinHandle<()>>,
}

impl Sink {
    /// Opens the default PulseAudio sink on a thread that feeds it `source`.
    ///
    /// Returns once the server has accepted the stream, so a host without a
    /// PulseAudio library or server reports the reason to the caller rather
    /// than playing silence.
    pub fn start(source: MixerSource) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let (ready, opened) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("pulse-out".to_string())
            .spawn(move || feed(source, flag, ready))
            .map_err(|error| format!("cannot start the PulseAudio thread: {error}"))?;
        match opened.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err("the PulseAudio thread stopped before it opened".to_string())
            }
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Opens the stream, reports whether it did, and then keeps it fed.
fn feed(
    mut source: MixerSource,
    stop: Arc<AtomicBool>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let library = unsafe { libloading::Library::new("libpulse-simple.so.0") }
        .or_else(|_| unsafe { libloading::Library::new("libpulse-simple.so") });
    let library = match library {
        Ok(library) => library,
        Err(error) => {
            let _ = ready.send(Err(format!("no PulseAudio client library: {error}")));
            return;
        }
    };
    let symbols = unsafe {
        (
            library
                .get::<SimpleNew>(b"pa_simple_new\0")
                .map(|found| *found),
            library
                .get::<SimpleWrite>(b"pa_simple_write\0")
                .map(|found| *found),
            library
                .get::<SimpleFree>(b"pa_simple_free\0")
                .map(|found| *found),
        )
    };
    let (open, write, free) = match symbols {
        (Ok(open), Ok(write), Ok(free)) => (open, write, free),
        _ => {
            let _ = ready.send(Err("the PulseAudio library is missing a symbol".to_string()));
            return;
        }
    };
    let strerror = unsafe { library.get::<StrError>(b"pa_strerror\0") }
        .ok()
        .map(|found| *found);

    let name = CString::new("blue-dove").expect("the app name holds no nul");
    let stream = CString::new("Playback").expect("the stream name holds no nul");
    let spec = SampleSpec {
        format: FLOAT32,
        rate: RATE,
        channels: CHANNELS as u8,
    };
    let mut error = 0;
    let handle = unsafe {
        open(
            std::ptr::null(),
            name.as_ptr(),
            PLAYBACK,
            std::ptr::null(),
            stream.as_ptr(),
            &spec,
            std::ptr::null(),
            std::ptr::null(),
            &mut error,
        )
    };
    if handle.is_null() {
        let _ = ready.send(Err(format!(
            "cannot open PulseAudio: {}",
            describe(strerror, error)
        )));
        return;
    }
    if ready.send(Ok(())).is_err() {
        unsafe { free(handle) };
        return;
    }

    let mut buffer = vec![0.0_f32; FRAMES * usize::from(CHANNELS)];
    while !stop.load(Ordering::Relaxed) {
        for sample in &mut buffer {
            *sample = source.next().unwrap_or(0.0);
        }
        let mut error = 0;
        let written = unsafe {
            write(
                handle,
                buffer.as_ptr().cast(),
                buffer.len() * size_of::<f32>(),
                &mut error,
            )
        };
        if written < 0 {
            break;
        }
    }
    unsafe { free(handle) };
}

/// The server's words for an error code, when the library offers them.
fn describe(strerror: Option<StrError>, error: c_int) -> String {
    let Some(strerror) = strerror else {
        return format!("error {error}");
    };
    let text = unsafe { strerror(error) };
    if text.is_null() {
        return format!("error {error}");
    }
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn an_error_without_a_message_still_names_its_code() {
        assert_eq!(describe(None, 7), "error 7");
    }
}
