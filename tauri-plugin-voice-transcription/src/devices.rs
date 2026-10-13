//! Which microphone Juno listens on.
//!
//! cpal identifies a device by its name, so the name is also the stored
//! choice. The person's choice lives here rather than in the controllers
//! because both capture paths (dictation and always-listening) need it, and
//! both read it on the worker thread at the moment they open the stream. That
//! is the only moment it matters: a device unplugged between sessions is
//! noticed on the next one, not remembered as broken.

use crate::capture_failure::CaptureStartFailure;
use crate::utils::{downmix_f32_to_mono, downmix_i16_to_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

/// One microphone, as the settings window lists it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    /// The device name, which is also its id.
    pub name: String,
    /// True for the one macOS would choose on its own.
    pub is_default: bool,
}

/// The microphone the person picked, or `None` for "whatever macOS is using".
///
/// A plain `None` default matters: an install that has never opened Audio
/// settings follows the system, which is what it did before this existed.
static PREFERRED_INPUT: Mutex<Option<String>> = Mutex::new(None);

/// Remember the microphone the person picked. `None` means follow the system.
pub fn set_preferred_input_device(name: Option<String>) {
    let cleaned = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    match PREFERRED_INPUT.lock() {
        Ok(mut preferred) => {
            tracing::info!(
                "[VoiceDevices] Microphone preference set to {}",
                cleaned.as_deref().unwrap_or("the system default")
            );
            *preferred = cleaned;
        }
        Err(e) => tracing::error!("[VoiceDevices] Could not store the microphone choice: {e}"),
    }
    // Get the new one ready before the next press, not during it.
    request_warm();
}

/// The microphone the person picked, if any.
pub fn preferred_input_device() -> Option<String> {
    match PREFERRED_INPUT.lock() {
        Ok(preferred) => preferred.clone(),
        Err(e) => {
            tracing::error!("[VoiceDevices] Could not read the microphone choice: {e}");
            None
        }
    }
}

/// Every microphone this machine has, default first flagged.
///
/// Read-only: listing devices never opens one, so it does not ask for
/// microphone access and makes no sound.
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let Ok(devices) = host.input_devices() else {
        tracing::warn!("[VoiceDevices] The system would not list input devices");
        return Vec::new();
    };

    let mut listed = Vec::new();
    let mut seen = HashSet::new();
    for device in devices {
        let Ok(name) = device.name() else { continue };
        if !seen.insert(name.clone()) {
            continue;
        }
        let is_default = default_name.as_deref() == Some(name.as_str());
        listed.push(AudioDeviceInfo { name, is_default });
    }
    listed
}

/// What listening settled on, which is not always what was asked for.
pub struct ResolvedInput {
    pub device: cpal::Device,
    /// The device actually opened.
    pub name: String,
    /// Set when the chosen device was not there and the system default stood
    /// in for it. Unplugged headphones must not brick dictation.
    pub substituted_for: Option<String>,
}

/// The decision, separated from the hardware so it can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputChoice {
    /// The device the person picked, and it is here.
    Chosen(String),
    /// The chosen device is gone; this is the system default standing in.
    Substituted { requested: String, used: String },
    /// No preference, so whatever macOS is using.
    SystemDefault(String),
    /// Nothing to listen on.
    Nothing { requested: Option<String> },
}

/// Which microphone to open, given the preference, what is plugged in, and
/// what the system would pick.
///
/// Pure, because the fallback is the part that matters and it must be provable
/// without a microphone.
pub(crate) fn choose_input(
    preferred: Option<&str>,
    available: &[String],
    system_default: Option<&str>,
) -> InputChoice {
    let wanted = preferred.map(str::trim).filter(|w| !w.is_empty());

    if let Some(wanted) = wanted {
        if available.iter().any(|name| name == wanted) {
            return InputChoice::Chosen(wanted.to_string());
        }
        // The chosen device went away. Fall back rather than refusing to
        // listen, and remember what was asked for so the person is told.
        return match system_default.filter(|d| !d.is_empty()) {
            Some(default) => InputChoice::Substituted {
                requested: wanted.to_string(),
                used: default.to_string(),
            },
            None => InputChoice::Nothing {
                requested: Some(wanted.to_string()),
            },
        };
    }

    match system_default.filter(|d| !d.is_empty()) {
        Some(default) => InputChoice::SystemDefault(default.to_string()),
        None => InputChoice::Nothing { requested: None },
    }
}

/// Fallback label for a device whose name the system will not give us. It is
/// still openable, so refusing to listen over a missing string would be worse.
const UNNAMED_DEVICE: &str = "your microphone";

/// Pull a device out of the enumerated list by name.
fn take_device(present: &mut Vec<(String, cpal::Device)>, wanted: &str) -> Option<cpal::Device> {
    present
        .iter()
        .position(|(name, _)| name == wanted)
        .map(|index| present.remove(index).1)
}

/// The handle to open for `wanted`: cpal's own default-device handle when
/// `wanted` is the system default, the enumerated one otherwise.
///
/// Which handle matters. cpal 0.15 on macOS gives a stream opened from an
/// enumerated device (`is_default: false`) a disconnect listener that holds a
/// strong clone of the stream inside the stream, so dropping the stream never
/// frees it. A stream from the default-device handle gets no listener and is
/// freed when dropped. [`LiveInput`] stops the device either way; this keeps
/// the common case from also leaking the stopped audio unit.
fn take_for_opening(
    present: &mut Vec<(String, cpal::Device)>,
    default: &mut Option<(String, cpal::Device)>,
    wanted: &str,
) -> Option<cpal::Device> {
    if default.as_ref().is_some_and(|(name, _)| name == wanted) {
        return default.take().map(|(_, device)| device);
    }
    take_device(present, wanted)
}

/// Open the microphone Juno should listen on.
pub fn resolve_input_device(
    preferred: Option<&str>,
) -> std::result::Result<ResolvedInput, CaptureStartFailure> {
    let host = cpal::default_host();

    // A device whose name the system will not give us is still openable, so
    // it gets a label rather than being treated as absent.
    let mut default = host.default_input_device().map(|d| {
        let name = d.name().unwrap_or_else(|_| UNNAMED_DEVICE.to_string());
        (name, d)
    });

    // Following the system, or the choice is the system's own microphone: no
    // list needed. Listing asks every device on the Mac, virtual drivers
    // included, for its formats, and that took up to three seconds a press
    // while the person was already talking.
    let wanted = preferred.map(str::trim).filter(|w| !w.is_empty());
    if let Some((name, device)) =
        default.take_if(|(name, _)| wanted.is_none_or(|w| w == name.as_str()))
    {
        return Ok(ResolvedInput {
            device,
            name,
            substituted_for: None,
        });
    }

    let mut present: Vec<(String, cpal::Device)> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                present.push((name, device));
            }
        }
    }
    let default_name = default.as_ref().map(|(name, _)| name.clone());
    let names: Vec<String> = present.iter().map(|(name, _)| name.clone()).collect();

    match choose_input(preferred, &names, default_name.as_deref()) {
        InputChoice::Chosen(name) => {
            let device = take_for_opening(&mut present, &mut default, &name).ok_or_else(|| {
                CaptureStartFailure::ChosenDeviceGone {
                    requested: name.clone(),
                }
            })?;
            Ok(ResolvedInput {
                device,
                name,
                substituted_for: None,
            })
        }
        InputChoice::Substituted { requested, used } => {
            let device = take_for_opening(&mut present, &mut default, &used)
                .or_else(|| default.take().map(|(_, device)| device))
                .ok_or_else(|| CaptureStartFailure::ChosenDeviceGone {
                    requested: requested.clone(),
                })?;
            tracing::warn!(
                "[VoiceDevices] {requested} is not connected; listening on {used} instead"
            );
            Ok(ResolvedInput {
                device,
                name: used,
                substituted_for: Some(requested),
            })
        }
        InputChoice::SystemDefault(name) => {
            let device = take_for_opening(&mut present, &mut default, &name)
                .or_else(|| default.take().map(|(_, device)| device))
                .ok_or(CaptureStartFailure::NoInputDevice)?;
            Ok(ResolvedInput {
                device,
                name,
                substituted_for: None,
            })
        }
        InputChoice::Nothing { requested } => Err(match requested {
            Some(requested) => CaptureStartFailure::ChosenDeviceGone { requested },
            None => CaptureStartFailure::NoInputDevice,
        }),
    }
}

/// Something that keeps an input device running until it is halted.
///
/// A running input is what macOS draws the orange microphone indicator for.
/// The real one is a cpal stream; tests stand in a fake that, like cpal on
/// macOS, keeps running when it is merely dropped.
pub(crate) trait InputHalt {
    /// Stop the device. Safe to call more than once.
    fn halt(&self);
}

impl InputHalt for cpal::Stream {
    fn halt(&self) {
        if let Err(e) = self.pause() {
            tracing::warn!("[VoiceDevices] Could not stop the input stream: {e}");
        }
    }
}

/// Every [`LiveInput`] the process has open right now, dictation and
/// always-listening together.
fn open_inputs() -> &'static Arc<AtomicUsize> {
    static OPEN: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
    OPEN.get_or_init(|| Arc::new(AtomicUsize::new(0)))
}

/// How many microphone input streams are running in this process.
///
/// Zero whenever nothing is recording and always-listening is off. Anything
/// else is a hot microphone behind an idle bar.
pub fn open_input_streams() -> usize {
    open_inputs().load(Ordering::SeqCst)
}

/// Inputs running that nothing accounts for. Once a recording has ended
/// (dictation not running) the only input allowed to remain is the one
/// always-listening holds; with always-listening off that is none.
pub fn stray_input_streams(open: usize, dictation_captures: usize, wake_captures: usize) -> usize {
    open.saturating_sub(dictation_captures + wake_captures)
}

/// An open microphone input that is stopped when it is dropped.
///
/// Dropping a cpal stream is not enough on macOS. cpal 0.15 attaches a
/// disconnect listener to any stream opened from an enumerated device, and
/// that listener owns a clone of the stream, stored inside the stream. The
/// cycle means the drop frees nothing: the audio unit keeps running, its
/// callback keeps firing into a closed channel, and the orange indicator stays
/// up after every cancel and every send. Since #652 every capture opened an
/// enumerated device. So the stop is explicit here, and counted, rather than
/// left to a destructor that never runs.
pub struct LiveInput {
    stream: Option<Box<dyn InputHalt>>,
    open: Arc<AtomicUsize>,
}

impl LiveInput {
    /// Wrap a started stream, counted in [`open_input_streams`].
    pub(crate) fn new(stream: Box<dyn InputHalt>) -> Self {
        Self::counted(stream, Arc::clone(open_inputs()))
    }

    /// Wrap a started stream, counted in `open`. Tests pass their own counter
    /// so parallel tests do not read each other's microphones.
    pub(crate) fn counted(stream: Box<dyn InputHalt>, open: Arc<AtomicUsize>) -> Self {
        open.fetch_add(1, Ordering::SeqCst);
        Self {
            stream: Some(stream),
            open,
        }
    }
}

impl Drop for LiveInput {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            // Stop first: dropping alone does not stop a cpal stream that
            // owns a clone of itself.
            stream.halt();
            drop(stream);
            self.open.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Open a microphone and start it, sending mono f32 chunks down `audio_data_tx`.
///
/// Any channel count is averaged to mono (see
/// [`crate::utils::downmix_f32_to_mono`]). There is deliberately no channel
/// filter: rejecting a three-channel device is what silently killed
/// always-listening before, and the person's own default microphone reports
/// three channels.
pub fn start_mono_stream(
    device: &cpal::Device,
    device_name: &str,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    audio_data_tx: Sender<Vec<f32>>,
) -> std::result::Result<LiveInput, CaptureStartFailure> {
    let stream = build_stream(
        device,
        device_name,
        config,
        sample_format,
        move |mono| {
            if let Err(e) = audio_data_tx.send(mono) {
                tracing::error!("[VoiceDevices] Could not pass on audio: {e}");
            }
        },
        |err| tracing::error!("[VoiceDevices] Input stream error: {err}"),
    )?;
    start(&stream, device_name)?;
    Ok(LiveInput::new(Box::new(stream)))
}

/// Build an input stream that hands each buffer, downmixed to mono, to
/// `deliver`. It is built stopped (see `vendor/cpal/JUNO_PATCH.md`): nothing
/// is recorded and no indicator shows until [`start`].
fn build_stream(
    device: &cpal::Device,
    device_name: &str,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    deliver: impl Fn(Vec<f32>) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> std::result::Result<cpal::Stream, CaptureStartFailure> {
    let channels = config.channels as usize;
    if channels > 1 {
        tracing::info!(
            "[VoiceDevices] {device_name} reports {channels} channels; averaging down to mono"
        );
    }

    let built = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                deliver(downmix_f32_to_mono(data, channels))
            },
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                deliver(downmix_i16_to_mono(data, channels))
            },
            on_error,
            None,
        ),
        other => {
            return Err(CaptureStartFailure::UnsupportedSampleFormat {
                device: device_name.to_string(),
                format: format!("{other:?}"),
            })
        }
    };

    built.map_err(|e| CaptureStartFailure::StreamBuildFailed {
        device: device_name.to_string(),
        detail: format!("{e:?}"),
    })
}

/// Start a built stream. A failed start is stopped again, in case the device
/// got as far as running.
fn start(stream: &cpal::Stream, device_name: &str) -> std::result::Result<(), CaptureStartFailure> {
    stream.play().map_err(|e| {
        stream.halt();
        CaptureStartFailure::StreamStartFailed {
            device: device_name.to_string(),
            detail: format!("{e:?}"),
        }
    })
}

/// The format to record in: the device's own. Opening it then never changes
/// its rate under other apps, and a rate another app sets later is a change
/// [`keep_microphone_warm`] hears about. The final decode resamples anyway.
pub fn input_config(
    device: &cpal::Device,
    device_name: &str,
) -> std::result::Result<(cpal::StreamConfig, SampleFormat), CaptureStartFailure> {
    let native = device.default_input_config().map_err(|e| {
        CaptureStartFailure::DeviceConfigUnavailable {
            device: device_name.to_string(),
            detail: format!("{e:?}"),
        }
    })?;
    if matches!(
        native.sample_format(),
        SampleFormat::F32 | SampleFormat::I16
    ) {
        return Ok((native.config(), native.sample_format()));
    }

    // The device's own format is one we cannot read: the same rate in one we
    // can.
    let rate = native.sample_rate();
    device
        .supported_input_configs()
        .ok()
        .and_then(|mut ranges| {
            ranges.find(|r| {
                matches!(r.sample_format(), SampleFormat::F32 | SampleFormat::I16)
                    && (r.min_sample_rate()..=r.max_sample_rate()).contains(&rate)
            })
        })
        .map(|range| {
            let config = range.with_sample_rate(rate);
            (config.config(), config.sample_format())
        })
        .ok_or_else(|| CaptureStartFailure::UnsupportedSampleFormat {
            device: device_name.to_string(),
            format: format!("{:?}", native.sample_format()),
        })
}

/// Where a warm stream's audio goes: the recording that holds it, or nowhere
/// while it waits.
type AudioSink = Arc<Mutex<Option<Sender<Vec<f32>>>>>;

/// cpal marks every `Stream` `!Send` because some backends are bound to the
/// thread that made them. CoreAudio's is not: it is an `Arc<Mutex<..>>` around
/// an audio unit that may be started, stopped and disposed from any thread.
/// Juno's capture only runs on macOS, which is what makes handing one from the
/// thread that built it to the recording that runs it sound.
struct SendStream(cpal::Stream);

// SAFETY: see the type's doc comment. The stream is only ever used by one
// thread at a time, under `WARM_INPUT` or by the recording that took it.
unsafe impl Send for SendStream {}

/// The dictation microphone, built and stopped, waiting for the key.
///
/// It is built before anyone presses anything: at launch, whenever the Mac's
/// microphones change (one plugged in or out, a new default, a new rate), and
/// whenever the person picks a different one. A press only starts it, which
/// takes milliseconds; building one took up to three seconds, while the
/// person was already talking. Built and stopped is not a hot microphone: the
/// device is not running, macOS draws no orange indicator, and
/// [`open_input_streams`] does not count it. When a recording ends its stream
/// is stopped and waits again.
pub struct WarmInput {
    device: cpal::Device,
    name: String,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    stream: SendStream,
    sink: AudioSink,
    /// Set by the stream's error callback (the device went away). A broken
    /// stream is never started.
    broken: Arc<AtomicBool>,
    /// Hears another app changing this device's rate, so the stream is
    /// rebuilt before the next press rather than during it.
    _rate_watch: Option<watch::RateWatch>,
}

static WARM_INPUT: Mutex<Option<WarmInput>> = Mutex::new(None);

impl WarmInput {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    pub fn channels(&self) -> u16 {
        self.config.channels
    }

    /// True while it is still the device's current rate: a stream built for
    /// another rate would record garbage.
    fn rate_is_current(&self) -> bool {
        self.device
            .default_input_config()
            .is_ok_and(|current| current.sample_rate() == self.config.sample_rate)
    }

    /// Start it, delivering to `audio_data_tx`. A stream that will not start
    /// (CoreAudio restarted under it) is rebuilt on the same device at the
    /// same rate, so the recording that claimed it never pays for it.
    pub fn resume(
        self,
        audio_data_tx: Sender<Vec<f32>>,
    ) -> std::result::Result<LiveInput, CaptureStartFailure> {
        let device = self.device.clone();
        let name = self.name.clone();
        let config = self.config.clone();
        let sample_format = self.sample_format;
        match self.lend(audio_data_tx.clone()) {
            Ok(input) => Ok(input),
            Err(failure) => {
                tracing::warn!(
                    "[VoiceDevices] {name} would not start ({}); building it again",
                    failure.message()
                );
                build_warm(device, name, config, sample_format)?.lend(audio_data_tx)
            }
        }
    }

    /// Start it for one recording. Dropping the returned input stops it and
    /// puts it back to wait for the next press.
    fn lend(
        self,
        audio_data_tx: Sender<Vec<f32>>,
    ) -> std::result::Result<LiveInput, CaptureStartFailure> {
        set_sink(&self.sink, Some(audio_data_tx));
        if let Err(failure) = start(&self.stream.0, &self.name) {
            set_sink(&self.sink, None);
            return Err(failure);
        }
        Ok(LiveInput::new(Box::new(WarmLease(Some(self)))))
    }
}

fn set_sink(sink: &AudioSink, tx: Option<Sender<Vec<f32>>>) {
    match sink.lock() {
        Ok(mut slot) => *slot = tx,
        Err(e) => tracing::error!("[VoiceDevices] Could not route microphone audio: {e}"),
    }
}

/// A warm stream lent to one recording. Halting it stops the device; dropping
/// it puts the stopped stream back for the next press.
struct WarmLease(Option<WarmInput>);

impl InputHalt for WarmLease {
    fn halt(&self) {
        if let Some(warm) = &self.0 {
            warm.stream.0.halt();
            set_sink(&warm.sink, None);
        }
    }
}

impl Drop for WarmLease {
    fn drop(&mut self) {
        if let Some(warm) = self.0.take() {
            put_back(warm);
        }
    }
}

/// A recording is over: its stream waits for the next press, unless the
/// device went away or a newer one was built while it recorded (the Mac's
/// microphones changed mid-sentence), in which case the newer one stays.
fn put_back(warm: WarmInput) {
    if warm.broken.load(Ordering::SeqCst) {
        tracing::info!(
            "[VoiceDevices] {} went away; building the next one",
            warm.name
        );
        request_warm();
        return;
    }
    match WARM_INPUT.lock() {
        Ok(mut slot) => {
            if slot.is_none() {
                *slot = Some(warm);
            }
        }
        Err(e) => tracing::error!("[VoiceDevices] Could not keep the microphone ready: {e}"),
    }
    // A first dictation that had to ask for microphone access: from now on
    // the next one is ready before the press.
    keep_microphone_warm();
}

/// The stream waiting for this press, if it is still the microphone this
/// press should open, still there, and still at its rate.
///
/// Cheap on purpose: a few property reads on one device, never the device
/// list. Anything that does not match is dropped, this press opens the device
/// itself, and a fresh one is built for the next.
pub fn claim_warm_input(preferred: Option<&str>) -> Option<WarmInput> {
    let warm = WARM_INPUT.lock().ok()?.take();
    let Some(warm) = warm else {
        tracing::info!("[VoiceDevices] No microphone was ready for this press");
        request_warm();
        return None;
    };
    if warm.broken.load(Ordering::SeqCst) {
        request_warm();
        return None;
    }

    // Following the system means the device macOS has as its default right
    // now, compared by CoreAudio id: two microphones can share a name.
    let wanted = match preferred.map(str::trim).filter(|w| !w.is_empty()) {
        Some(wanted) => warm.name == wanted,
        None => cpal::default_host()
            .default_input_device()
            .is_some_and(|current| watch::same_device(&current, &warm.device)),
    };
    if !wanted || !warm.rate_is_current() {
        tracing::info!(
            "[VoiceDevices] {} is not the microphone this press needs; opening it directly",
            warm.name
        );
        request_warm();
        return None;
    }
    Some(warm)
}

/// Build a stopped stream on `device`, ready to be lent to a recording.
fn build_warm(
    device: cpal::Device,
    device_name: String,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
) -> std::result::Result<WarmInput, CaptureStartFailure> {
    let sink: AudioSink = Arc::new(Mutex::new(None));
    let broken = Arc::new(AtomicBool::new(false));

    let deliver_to = Arc::clone(&sink);
    let broken_by = Arc::clone(&broken);
    let stream = build_stream(
        &device,
        &device_name,
        &config,
        sample_format,
        move |mono| {
            if let Ok(slot) = deliver_to.lock() {
                if let Some(tx) = slot.as_ref() {
                    if let Err(e) = tx.send(mono) {
                        tracing::error!("[VoiceDevices] Could not pass on audio: {e}");
                    }
                }
            }
        },
        move |err| {
            tracing::error!("[VoiceDevices] Input stream error: {err}");
            if matches!(err, cpal::StreamError::DeviceNotAvailable) {
                broken_by.store(true, Ordering::SeqCst);
                request_warm();
            }
        },
    )?;

    Ok(WarmInput {
        _rate_watch: watch::RateWatch::new(&device),
        device,
        name: device_name,
        config,
        sample_format,
        stream: SendStream(stream),
        sink,
        broken,
    })
}

/// Open the dictation microphone for a press that found nothing ready, and
/// keep it ready afterwards.
pub fn start_warm_stream(
    device: cpal::Device,
    device_name: String,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    audio_data_tx: Sender<Vec<f32>>,
) -> std::result::Result<LiveInput, CaptureStartFailure> {
    build_warm(device, device_name, config, sample_format)?.lend(audio_data_tx)
}

/// Ask for the dictation microphone to be made ready, off the caller's thread.
///
/// Driven only by events. Requests that arrive while one is being served are
/// served together by one more pass, which finds the microphone already ready
/// unless something changed again; nothing waits on a clock for a burst of
/// changes to finish.
pub fn request_warm() {
    static WARMER: OnceLock<Option<Sender<()>>> = OnceLock::new();
    let warmer = WARMER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("juno-mic-ready".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.try_recv().is_ok() {}
                    ensure_warm();
                }
            })
            .map_err(|e| {
                tracing::error!("[VoiceDevices] Could not start the microphone warmer: {e}")
            })
            .ok()
            .map(|_| tx)
    });
    if let Some(tx) = warmer {
        let _ = tx.send(());
    }
}

/// From now on, keep the dictation microphone built and waiting: now, and
/// again whenever the Mac's microphones change. Safe to call more than once.
/// Only call it once microphone access is granted; building the device is
/// what would ask for it.
pub fn keep_microphone_warm() {
    static WATCHING: OnceLock<()> = OnceLock::new();
    WATCHING.get_or_init(|| {
        watch::system_changes();
        request_warm();
    });
}

/// Build the stream for the microphone the next press will open, unless the
/// one waiting already is.
fn ensure_warm() {
    use crate::mic_permissions::{check_microphone_permission, MicrophonePermissionStatus};
    if !matches!(
        check_microphone_permission(),
        MicrophonePermissionStatus::Granted | MicrophonePermissionStatus::NotApplicable
    ) {
        return;
    }

    let began = std::time::Instant::now();
    let resolved = match resolve_input_device(preferred_input_device().as_deref()) {
        Ok(resolved) => resolved,
        Err(failure) => {
            tracing::info!("[VoiceDevices] Nothing to get ready: {}", failure.message());
            if let Ok(mut slot) = WARM_INPUT.lock() {
                *slot = None;
            }
            return;
        }
    };

    if let Ok(slot) = WARM_INPUT.lock() {
        if let Some(waiting) = slot.as_ref() {
            if waiting.name == resolved.name
                && watch::same_device(&waiting.device, &resolved.device)
                && !waiting.broken.load(Ordering::SeqCst)
                && waiting.rate_is_current()
            {
                return;
            }
        }
    }

    let built = input_config(&resolved.device, &resolved.name).and_then(|(config, format)| {
        build_warm(resolved.device, resolved.name.clone(), config, format)
    });
    match built {
        Ok(warm) => {
            tracing::info!(
                "[VoiceDevices] {} is ready for the next press at {} Hz ({} ms)",
                warm.name,
                warm.sample_rate(),
                began.elapsed().as_millis()
            );
            match WARM_INPUT.lock() {
                Ok(mut slot) => *slot = Some(warm),
                Err(e) => {
                    tracing::error!("[VoiceDevices] Could not keep the microphone ready: {e}")
                }
            }
        }
        Err(failure) => tracing::warn!(
            "[VoiceDevices] Could not get {} ready: {}",
            resolved.name,
            failure.message()
        ),
    }
}

/// CoreAudio change notifications, each of which asks for the microphone to
/// be made ready again.
#[cfg(target_os = "macos")]
mod watch {
    use coreaudio::sys::{
        kAudioDevicePropertyNominalSampleRate, kAudioHardwarePropertyDefaultInputDevice,
        kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMaster,
        kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectAddPropertyListener,
        AudioObjectID, AudioObjectPropertyAddress, AudioObjectPropertySelector,
        AudioObjectRemovePropertyListener, OSStatus,
    };
    use std::ffi::c_void;
    use std::ptr::null_mut;

    unsafe extern "C" fn changed(
        _object: AudioObjectID,
        _count: u32,
        _addresses: *const AudioObjectPropertyAddress,
        _data: *mut c_void,
    ) -> OSStatus {
        super::request_warm();
        0
    }

    fn address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        }
    }

    fn listen(object: AudioObjectID, selector: AudioObjectPropertySelector) -> bool {
        let address = address(selector);
        // SAFETY: `changed` is a plain function with the listener signature,
        // and the address outlives the call (CoreAudio copies it).
        let status =
            unsafe { AudioObjectAddPropertyListener(object, &address, Some(changed), null_mut()) };
        if status != 0 {
            tracing::warn!("[VoiceDevices] Could not watch for microphone changes ({status})");
        }
        status == 0
    }

    /// A microphone plugged in or out, or a new default.
    pub fn system_changes() {
        listen(
            kAudioObjectSystemObject,
            kAudioHardwarePropertyDefaultInputDevice,
        );
        listen(kAudioObjectSystemObject, kAudioHardwarePropertyDevices);
    }

    fn device_id(device: &cpal::Device) -> AudioObjectID {
        let cpal::platform::DeviceInner::CoreAudio(device) = device.as_inner();
        device.audio_device_id()
    }

    /// The same physical device. Names are not enough: two microphones can
    /// share one, and a device unplugged and plugged back in gets a new id.
    pub fn same_device(a: &cpal::Device, b: &cpal::Device) -> bool {
        device_id(a) == device_id(b)
    }

    /// A rate change on one device, for as long as this is held.
    pub struct RateWatch(AudioObjectID);

    impl RateWatch {
        pub fn new(device: &cpal::Device) -> Option<Self> {
            let id = device_id(device);
            listen(id, kAudioDevicePropertyNominalSampleRate).then_some(Self(id))
        }
    }

    impl Drop for RateWatch {
        fn drop(&mut self) {
            let address = address(kAudioDevicePropertyNominalSampleRate);
            // SAFETY: the same object, address and function that were added.
            unsafe {
                AudioObjectRemovePropertyListener(self.0, &address, Some(changed), null_mut());
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod watch {
    use cpal::traits::DeviceTrait;

    pub fn system_changes() {}

    pub fn same_device(a: &cpal::Device, b: &cpal::Device) -> bool {
        a.name().ok() == b.name().ok()
    }

    pub struct RateWatch;

    impl RateWatch {
        pub fn new(_device: &cpal::Device) -> Option<Self> {
            None
        }
    }
}

/// The device the next capture will open, for the settings window to show
/// without opening anything.
pub fn effective_input_device_name() -> Option<String> {
    let preferred = preferred_input_device();
    let listed = list_input_devices();
    let names: Vec<String> = listed.iter().map(|d| d.name.clone()).collect();
    let default_name = listed
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.as_str());
    match choose_input(preferred.as_deref(), &names, default_name) {
        InputChoice::Chosen(name) => Some(name),
        InputChoice::Substituted { used, .. } => Some(used),
        InputChoice::SystemDefault(name) => Some(name),
        InputChoice::Nothing { .. } => None,
    }
}

/// A microphone for tests that behaves like cpal's on macOS: dropping it does
/// not stop it, only an explicit halt does.
#[cfg(test)]
pub(crate) mod fake_input {
    use super::{InputHalt, LiveInput};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Stands in for the device. `running` is what macOS would draw the
    /// orange indicator for. There is deliberately no `Drop` that clears it:
    /// the real stream holds a clone of itself, so its drop does nothing.
    pub(crate) struct CyclicInput {
        running: Arc<AtomicBool>,
    }

    impl InputHalt for CyclicInput {
        fn halt(&self) {
            self.running.store(false, Ordering::SeqCst);
        }
    }

    /// A started input, counted in `open`, whose device state is `running`.
    pub(crate) fn start(open: &Arc<AtomicUsize>, running: &Arc<AtomicBool>) -> LiveInput {
        running.store(true, Ordering::SeqCst);
        LiveInput::counted(
            Box::new(CyclicInput {
                running: Arc::clone(running),
            }),
            Arc::clone(open),
        )
    }

    /// The same device without the wrapper, the way capture held it before.
    pub(crate) fn start_unwrapped(running: &Arc<AtomicBool>) -> CyclicInput {
        running.store(true, Ordering::SeqCst);
        CyclicInput {
            running: Arc::clone(running),
        }
    }
}

#[cfg(test)]
mod live_input_tests {
    use super::fake_input;
    use super::stray_input_streams;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// The bug: dropping the stream was the whole of "stop", and on macOS a
    /// cpal stream from an enumerated device survives its drop. This pins
    /// that a bare drop is not enough, so the wrapper is what must stop it.
    #[test]
    fn a_dropped_stream_alone_keeps_the_microphone_running() {
        let running = Arc::new(AtomicBool::new(false));
        drop(fake_input::start_unwrapped(&running));
        assert!(running.load(Ordering::SeqCst));
    }

    #[test]
    fn dropping_a_live_input_stops_the_device_and_uncounts_it() {
        let open = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(false));
        let input = fake_input::start(&open, &running);
        assert_eq!(open.load(Ordering::SeqCst), 1);
        assert!(running.load(Ordering::SeqCst));

        drop(input);
        assert_eq!(open.load(Ordering::SeqCst), 0);
        assert!(!running.load(Ordering::SeqCst), "the device was stopped");
    }

    #[test]
    fn only_always_listening_may_hold_an_input_once_a_recording_ends() {
        assert_eq!(stray_input_streams(0, 0, 0), 0);
        assert_eq!(stray_input_streams(1, 0, 1), 0, "the wake engine's own");
        assert_eq!(stray_input_streams(1, 0, 0), 1, "a hot mic behind idle");
        assert_eq!(stray_input_streams(2, 0, 1), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_input, InputChoice, UNNAMED_DEVICE};

    fn devices() -> Vec<String> {
        vec![
            "MacBook Air Microphone".to_string(),
            "Shure MV7".to_string(),
        ]
    }

    #[test]
    fn no_preference_follows_the_system() {
        assert_eq!(
            choose_input(None, &devices(), Some("MacBook Air Microphone")),
            InputChoice::SystemDefault("MacBook Air Microphone".to_string())
        );
    }

    #[test]
    fn a_connected_choice_is_honoured() {
        assert_eq!(
            choose_input(
                Some("Shure MV7"),
                &devices(),
                Some("MacBook Air Microphone")
            ),
            InputChoice::Chosen("Shure MV7".to_string())
        );
    }

    /// Unplugging the headphones must not brick dictation.
    #[test]
    fn a_vanished_choice_falls_back_to_the_system_default() {
        assert_eq!(
            choose_input(
                Some("AirPods Pro"),
                &devices(),
                Some("MacBook Air Microphone")
            ),
            InputChoice::Substituted {
                requested: "AirPods Pro".to_string(),
                used: "MacBook Air Microphone".to_string(),
            }
        );
    }

    #[test]
    fn a_vanished_choice_with_nothing_left_is_a_failure() {
        assert_eq!(
            choose_input(Some("AirPods Pro"), &[], None),
            InputChoice::Nothing {
                requested: Some("AirPods Pro".to_string())
            }
        );
    }

    #[test]
    fn no_microphone_at_all_is_a_failure() {
        assert_eq!(
            choose_input(None, &[], None),
            InputChoice::Nothing { requested: None }
        );
    }

    /// A stored empty string is the same as no preference, not a device named
    /// "". Settings files written by hand have both.
    #[test]
    fn a_blank_preference_is_no_preference() {
        for blank in ["", "   "] {
            assert_eq!(
                choose_input(Some(blank), &devices(), Some("Shure MV7")),
                InputChoice::SystemDefault("Shure MV7".to_string()),
                "{blank:?} should mean no preference"
            );
        }
    }

    #[test]
    fn the_unnamed_device_label_reads_as_a_device() {
        // It is dropped into sentences such as "Juno could not open ...".
        assert!(!UNNAMED_DEVICE.is_empty());
        assert_eq!(UNNAMED_DEVICE, UNNAMED_DEVICE.to_lowercase());
    }
}
