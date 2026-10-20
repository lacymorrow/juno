//! Which microphone Juno listens on.
//!
//! A microphone is identified by its CoreAudio UID (see [`crate::device_identity`]),
//! never by its display name: two microphones can share a name, and a name can
//! change. The name travels along only to be shown. The person's choice lives
//! here rather than in the controllers because both capture paths (dictation
//! and always-listening) need it, and both read it on the worker thread at the
//! moment they open the stream. That is the only moment it matters: a device
//! unplugged between sessions is noticed on the next one, not remembered as
//! broken.

use crate::capture_failure::CaptureStartFailure;
use crate::device_identity::{device_ref, device_uid, DeviceChoice, DeviceRef};
use crate::utils::{downmix_f32_to_mono, downmix_i16_to_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

/// One microphone, as the settings window lists it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    /// The CoreAudio UID: what a choice is stored and matched by.
    pub id: String,
    /// What the person sees. Display only.
    pub name: String,
    /// True for the one macOS would choose on its own.
    pub is_default: bool,
}

impl AudioDeviceInfo {
    /// The device, without the flag.
    pub fn device_ref(&self) -> DeviceRef {
        DeviceRef {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

/// The microphone the person picked, or `None` for "whatever macOS is using".
///
/// A plain `None` default matters: an install that has never opened Audio
/// settings follows the system, which is what it did before this existed.
static PREFERRED_INPUT: Mutex<Option<DeviceChoice>> = Mutex::new(None);

/// Remember the microphone the person picked. `None` means follow the system.
pub fn set_preferred_input_device(choice: Option<DeviceChoice>) {
    let cleaned = choice.and_then(DeviceChoice::cleaned);
    match PREFERRED_INPUT.lock() {
        Ok(mut preferred) => {
            tracing::info!(
                "[VoiceDevices] Microphone preference set to {}",
                cleaned
                    .as_ref()
                    .map(DeviceChoice::label)
                    .unwrap_or_else(|| "the system default".to_string())
            );
            *preferred = cleaned;
        }
        Err(e) => tracing::error!("[VoiceDevices] Could not store the microphone choice: {e}"),
    }
    // Get the new one ready before the next press, not during it.
    microphones_changed();
}

/// The microphone the person picked, if any.
pub fn preferred_input_device() -> Option<DeviceChoice> {
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
    let default_id = host.default_input_device().map(|d| device_uid(&d));
    let Ok(devices) = host.input_devices() else {
        tracing::warn!("[VoiceDevices] The system would not list input devices");
        return Vec::new();
    };

    let mut listed = Vec::new();
    let mut seen = HashSet::new();
    for device in devices {
        let Some(found) = device_ref(&device) else {
            continue;
        };
        // By identity, so two microphones that share a name are both offered.
        if !seen.insert(found.id.clone()) {
            continue;
        }
        let is_default = default_id.as_deref() == Some(found.id.as_str());
        listed.push(AudioDeviceInfo {
            id: found.id,
            name: found.name,
            is_default,
        });
    }
    listed
}

/// What listening settled on, which is not always what was asked for.
pub struct ResolvedInput {
    pub device: cpal::Device,
    /// The UID of the device actually opened.
    pub id: String,
    /// The name of the device actually opened.
    pub name: String,
    /// Set when the chosen device was not there and the system default stood
    /// in for it, as the name the person chose it by. Unplugged headphones
    /// must not brick dictation.
    pub substituted_for: Option<String>,
}

/// The decision, separated from the hardware so it can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputChoice {
    /// The device the person picked, and it is here.
    Chosen(DeviceRef),
    /// The chosen device is gone; this is the system default standing in.
    Substituted { requested: String, used: DeviceRef },
    /// No preference, so whatever macOS is using.
    SystemDefault(DeviceRef),
    /// Nothing to listen on.
    Nothing { requested: Option<String> },
}

/// A choice that names something. A stored blank is the same as no choice.
fn named(choice: Option<&DeviceChoice>) -> Option<&DeviceChoice> {
    choice.filter(|w| w.uid.is_some() || !w.name.trim().is_empty())
}

/// Which microphone to open, given the preference, what is plugged in, and
/// what the system would pick.
///
/// Pure, because the fallback is the part that matters and it must be provable
/// without a microphone.
pub(crate) fn choose_input(
    preferred: Option<&DeviceChoice>,
    available: &[DeviceRef],
    system_default: Option<&DeviceRef>,
) -> InputChoice {
    if let Some(wanted) = named(preferred) {
        if let Some(found) = available.iter().find(|device| wanted.matches(device)) {
            return InputChoice::Chosen(found.clone());
        }
        // The chosen device went away. Fall back rather than refusing to
        // listen, and remember what was asked for so the person is told.
        return match system_default {
            Some(default) => InputChoice::Substituted {
                requested: wanted.label(),
                used: default.clone(),
            },
            None => InputChoice::Nothing {
                requested: Some(wanted.label()),
            },
        };
    }

    match system_default {
        Some(default) => InputChoice::SystemDefault(default.clone()),
        None => InputChoice::Nothing { requested: None },
    }
}

/// Fallback label for a device whose name the system will not give us. It is
/// still openable, so refusing to listen over a missing string would be worse.
const UNNAMED_DEVICE: &str = "your microphone";

/// A device paired with the handle that opens it.
type Handle = (DeviceRef, cpal::Device);

/// Pull a device out of the enumerated list by identity.
fn take_device(present: &mut Vec<Handle>, wanted_id: &str) -> Option<Handle> {
    present
        .iter()
        .position(|(found, _)| found.id == wanted_id)
        .map(|index| present.remove(index))
}

/// The handle to open for `wanted_id`: cpal's own default-device handle when
/// it is the system default, the enumerated one otherwise.
///
/// Which handle matters. cpal 0.15 on macOS gives a stream opened from an
/// enumerated device (`is_default: false`) a disconnect listener that holds a
/// strong clone of the stream inside the stream, so dropping the stream never
/// frees it. A stream from the default-device handle gets no listener and is
/// freed when dropped. [`LiveInput`] stops the device either way; this keeps
/// the common case from also leaking the stopped audio unit.
fn take_for_opening(
    present: &mut Vec<Handle>,
    default: &mut Option<Handle>,
    wanted_id: &str,
) -> Option<Handle> {
    if default
        .as_ref()
        .is_some_and(|(found, _)| found.id == wanted_id)
    {
        return default.take();
    }
    take_device(present, wanted_id)
}

fn resolved(handle: Handle, substituted_for: Option<String>) -> ResolvedInput {
    let (found, device) = handle;
    ResolvedInput {
        device,
        id: found.id,
        name: found.name,
        substituted_for,
    }
}

/// Open the microphone Juno should listen on.
pub fn resolve_input_device(
    preferred: Option<&DeviceChoice>,
) -> std::result::Result<ResolvedInput, CaptureStartFailure> {
    let host = cpal::default_host();

    // A device whose name the system will not give us is still openable, so
    // it gets a label rather than being treated as absent.
    let mut default: Option<Handle> = host.default_input_device().map(|d| {
        let name = d.name().unwrap_or_else(|_| UNNAMED_DEVICE.to_string());
        (
            DeviceRef {
                id: device_uid(&d),
                name,
            },
            d,
        )
    });

    // Following the system, or the choice is the system's own microphone: no
    // list needed. Listing asks every device on the Mac, virtual drivers
    // included, for its formats, and that took up to three seconds a press
    // while the person was already talking.
    let wanted = named(preferred);
    if let Some(handle) = default.take_if(|(found, _)| wanted.is_none_or(|w| w.matches(found))) {
        return Ok(resolved(handle, None));
    }

    let mut present: Vec<Handle> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Some(found) = device_ref(&device) {
                present.push((found, device));
            }
        }
    }
    let default_ref = default.as_ref().map(|(found, _)| found.clone());
    let refs: Vec<DeviceRef> = present.iter().map(|(found, _)| found.clone()).collect();

    match choose_input(preferred, &refs, default_ref.as_ref()) {
        InputChoice::Chosen(chosen) => {
            let handle =
                take_for_opening(&mut present, &mut default, &chosen.id).ok_or_else(|| {
                    CaptureStartFailure::ChosenDeviceGone {
                        requested: chosen.name.clone(),
                    }
                })?;
            Ok(resolved(handle, None))
        }
        InputChoice::Substituted { requested, used } => {
            let handle = take_for_opening(&mut present, &mut default, &used.id)
                .or_else(|| default.take())
                .ok_or_else(|| CaptureStartFailure::ChosenDeviceGone {
                    requested: requested.clone(),
                })?;
            tracing::warn!(
                "[VoiceDevices] {requested} is not connected; listening on {} instead",
                used.name
            );
            Ok(resolved(handle, Some(requested)))
        }
        InputChoice::SystemDefault(system) => {
            let handle = take_for_opening(&mut present, &mut default, &system.id)
                .or_else(|| default.take())
                .ok_or(CaptureStartFailure::NoInputDevice)?;
            Ok(resolved(handle, None))
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
    /// The device's UID, which is what "is this the microphone wanted" is
    /// decided on.
    id: String,
    name: String,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    stream: SendStream,
    sink: AudioSink,
    /// Set by the stream's error callback (the device went away). A broken
    /// stream is never started.
    broken: Arc<AtomicBool>,
    /// Hears another app changing this device's rate, so the stream is
    /// rebuilt before the next press rather than during it. Without it a
    /// rate change would go unheard, so the press checks the rate itself.
    rate_watch: Option<watch::RateWatch>,
}

static WARM_INPUT: Mutex<Option<WarmInput>> = Mutex::new(None);

impl WarmInput {
    pub fn name(&self) -> &str {
        &self.name
    }

    fn device_ref(&self) -> DeviceRef {
        DeviceRef {
            id: self.id.clone(),
            name: self.name.clone(),
        }
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
/// Free when nothing changed since the warmer last checked it: every change
/// that could make it the wrong stream is an event that marks it stale (see
/// [`Freshness`]), so a fresh one is taken without asking CoreAudio anything.
/// Those questions are IPC to the audio server and took half a second on a
/// press on some Macs. Only a stale stream is checked here, with a few
/// property reads on one device, never the device list. Anything that does
/// not match is dropped, this press opens the device itself, and a fresh one
/// is built for the next.
pub fn claim_warm_input(preferred: Option<&DeviceChoice>) -> Option<WarmInput> {
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

    // Trusted only while every change that could stale it is heard: a
    // listener CoreAudio refused would leave a change unannounced.
    let fresh = FRESHNESS.is_current()
        && SYSTEM_WATCHED.load(Ordering::SeqCst)
        && warm.rate_watch.is_some();
    // Following the system means the device macOS has as its default right
    // now, compared by CoreAudio id: two microphones can share a name. A
    // fresh stream was built for that default after the last change to it.
    let wanted = match named(preferred) {
        Some(wanted) => wanted.matches(&warm.device_ref()),
        None if fresh => true,
        None => cpal::default_host()
            .default_input_device()
            .is_some_and(|current| watch::same_device(&current, &warm.device)),
    };
    // A stream built for another rate would record garbage.
    let at_rate = fresh || warm.rate_is_current();
    if !wanted || !at_rate {
        tracing::info!(
            "[VoiceDevices] {} is not the microphone this press needs; opening it directly",
            warm.name
        );
        request_warm();
        return None;
    }
    Some(warm)
}

/// Whether the stream waiting for the press was checked against the Mac's
/// microphones as they are now.
///
/// A generation count rather than a flag. Every change that could make the
/// waiting stream the wrong one (a new default, a microphone plugged in or
/// out, a new rate on the held device, the device going away, a new
/// preference) bumps `changes` before it asks the warmer to run. The warmer
/// reads `changes` before it looks at anything and records that value once
/// the stream in the slot is verified or rebuilt. A change that lands while
/// it works is newer than what it records, so the stream stays stale until
/// the pass that change asked for catches up; a flag cleared at the end of a
/// pass would lose it.
struct Freshness {
    changes: AtomicU64,
    verified: AtomicU64,
}

impl Freshness {
    /// Stale until the first pass verifies something.
    const fn new() -> Self {
        Self {
            changes: AtomicU64::new(1),
            verified: AtomicU64::new(0),
        }
    }

    /// Something changed; whatever is waiting has to be checked again.
    fn changed(&self) {
        self.changes.fetch_add(1, Ordering::SeqCst);
    }

    /// The generation a pass starts from. Read before anything it checks.
    fn generation(&self) -> u64 {
        self.changes.load(Ordering::SeqCst)
    }

    /// The waiting stream matches the Mac as it was at `generation`. Never
    /// moves backwards, so a slow pass cannot undo a newer one's record.
    fn verified_at(&self, generation: u64) {
        self.verified.fetch_max(generation, Ordering::SeqCst);
    }

    /// Nothing has changed since the waiting stream was last verified.
    fn is_current(&self) -> bool {
        self.verified.load(Ordering::SeqCst) == self.changes.load(Ordering::SeqCst)
    }
}

static FRESHNESS: Freshness = Freshness::new();

/// CoreAudio accepted the listeners for a new default microphone and for one
/// plugged in or out, so those changes reach [`FRESHNESS`].
static SYSTEM_WATCHED: AtomicBool = AtomicBool::new(false);

/// Something about the Mac's microphones changed: mark the waiting stream
/// stale, then ask for it to be made ready again. In that order, so a press
/// in between checks it rather than trusting it.
fn microphones_changed() {
    FRESHNESS.changed();
    request_warm();
}

/// What to do when the Mac's audio hardware changes, so a window showing the
/// device list can be sent the list as it is now.
///
/// A callback rather than an `AppHandle`: this crate is generic over the Tauri
/// runtime and has no window of its own, and the notification arrives on a
/// CoreAudio thread. The host app installs what to do and keeps its own
/// handle.
static HARDWARE_CHANGED: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Have `report` called whenever a device is plugged in or unplugged, or
/// macOS picks a different default. Call once, during setup.
pub fn on_hardware_change(report: impl Fn() + Send + Sync + 'static) {
    if HARDWARE_CHANGED.set(Box::new(report)).is_err() {
        tracing::warn!("[VoiceDevices] Something is already listening for hardware changes");
    }
}

/// Tell the host the hardware changed.
///
/// Separate from the microphone's own freshness on purpose: what a settings
/// window is told does not depend on the warm stream, and a change that only
/// the speaker list cares about reports without asking for a microphone to be
/// rebuilt.
fn report_hardware_change() {
    if let Some(report) = HARDWARE_CHANGED.get() {
        report();
    }
}

/// A microphone came or went, or macOS picked a different input. Two things
/// are out of date: the microphone waiting for the next press, and any list of
/// devices on screen.
fn hardware_changed() {
    microphones_changed();
    report_hardware_change();
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
                microphones_changed();
            }
        },
    )?;

    Ok(WarmInput {
        rate_watch: watch::RateWatch::new(&device),
        id: device_uid(&device),
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

/// Hear the Mac's audio hardware changing: a device plugged in or unplugged,
/// and a new system default. Safe to call more than once.
///
/// Deliberately **not** gated on microphone permission, unlike
/// [`keep_microphone_warm`]. Asking CoreAudio to report a change opens no
/// device, so it needs no permission and makes no sound, and a settings window
/// listing devices has to stay right on a Mac where dictation cannot run yet.
pub fn watch_audio_hardware() {
    static WATCHING: OnceLock<()> = OnceLock::new();
    WATCHING.get_or_init(|| {
        SYSTEM_WATCHED.store(watch::system_changes(), Ordering::SeqCst);
    });
}

/// From now on, keep the dictation microphone built and waiting: now, and
/// again whenever the Mac's microphones change. Safe to call more than once.
/// Only call it once microphone access is granted; building the device is
/// what would ask for it. The watch itself does not wait on permission.
pub fn keep_microphone_warm() {
    static WATCHING: OnceLock<()> = OnceLock::new();
    WATCHING.get_or_init(|| {
        watch_audio_hardware();
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

    // Before anything is read, so a change that lands during this pass is
    // newer than what it records.
    let generation = FRESHNESS.generation();
    let began = std::time::Instant::now();
    let resolved = match resolve_input_device(preferred_input_device().as_ref()) {
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
            if waiting.id == resolved.id
                && watch::same_device(&waiting.device, &resolved.device)
                && !waiting.broken.load(Ordering::SeqCst)
                && waiting.rate_is_current()
            {
                FRESHNESS.verified_at(generation);
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
                Ok(mut slot) => {
                    *slot = Some(warm);
                    FRESHNESS.verified_at(generation);
                }
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
        kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
        kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeGlobal,
        kAudioObjectSystemObject, AudioObjectAddPropertyListener, AudioObjectID,
        AudioObjectPropertyAddress, AudioObjectPropertySelector, AudioObjectRemovePropertyListener,
        OSStatus,
    };
    use std::ffi::c_void;
    use std::ptr::null_mut;

    /// The signature CoreAudio calls back on. Named so each property can be
    /// watched with the callback that matches what changing it means.
    type Listener = unsafe extern "C" fn(
        AudioObjectID,
        u32,
        *const AudioObjectPropertyAddress,
        *mut c_void,
    ) -> OSStatus;

    /// A rate change on the one device the warm stream is holding. Nothing
    /// outside the microphone is affected, so nothing outside it is told.
    unsafe extern "C" fn rate_changed(
        _object: AudioObjectID,
        _count: u32,
        _addresses: *const AudioObjectPropertyAddress,
        _data: *mut c_void,
    ) -> OSStatus {
        super::microphones_changed();
        0
    }

    /// A device plugged in or unplugged, or a new default input.
    unsafe extern "C" fn hardware_changed(
        _object: AudioObjectID,
        _count: u32,
        _addresses: *const AudioObjectPropertyAddress,
        _data: *mut c_void,
    ) -> OSStatus {
        super::hardware_changed();
        0
    }

    /// A new default speaker. The list on screen is out of date; the
    /// microphone waiting for the next press is not, so it is left alone.
    unsafe extern "C" fn output_changed(
        _object: AudioObjectID,
        _count: u32,
        _addresses: *const AudioObjectPropertyAddress,
        _data: *mut c_void,
    ) -> OSStatus {
        super::report_hardware_change();
        0
    }

    fn address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        }
    }

    fn listen(
        object: AudioObjectID,
        selector: AudioObjectPropertySelector,
        on_change: Listener,
    ) -> bool {
        let address = address(selector);
        // SAFETY: `on_change` is a plain function with the listener signature,
        // and the address outlives the call (CoreAudio copies it).
        let status = unsafe {
            AudioObjectAddPropertyListener(object, &address, Some(on_change), null_mut())
        };
        if status != 0 {
            tracing::warn!("[VoiceDevices] Could not watch for a device change ({status})");
        }
        status == 0
    }

    /// A device plugged in or out, and a new default either way. The returned
    /// bool answers only the microphone's question: are both the changes that
    /// could stale the warm stream being heard.
    pub fn system_changes() -> bool {
        let default_input = listen(
            kAudioObjectSystemObject,
            kAudioHardwarePropertyDefaultInputDevice,
            hardware_changed,
        );
        let devices = listen(
            kAudioObjectSystemObject,
            kAudioHardwarePropertyDevices,
            hardware_changed,
        );
        // The speaker half of the settings list moves with the default output,
        // which the microphone does not care about. Kept out of the answer so
        // a failure to watch it cannot make the warm stream distrust itself.
        listen(
            kAudioObjectSystemObject,
            kAudioHardwarePropertyDefaultOutputDevice,
            output_changed,
        );
        default_input && devices
    }

    fn device_id(device: &cpal::Device) -> AudioObjectID {
        crate::device_identity::object_id(device)
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
            listen(id, kAudioDevicePropertyNominalSampleRate, rate_changed).then_some(Self(id))
        }
    }

    impl Drop for RateWatch {
        fn drop(&mut self) {
            let address = address(kAudioDevicePropertyNominalSampleRate);
            // SAFETY: the same object, address and function that were added.
            unsafe {
                AudioObjectRemovePropertyListener(self.0, &address, Some(rate_changed), null_mut());
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod watch {
    use cpal::traits::DeviceTrait;

    /// Nothing is heard here, so every press checks.
    pub fn system_changes() -> bool {
        false
    }

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
    let available: Vec<DeviceRef> = listed.iter().map(AudioDeviceInfo::device_ref).collect();
    let default = listed
        .iter()
        .find(|d| d.is_default)
        .map(AudioDeviceInfo::device_ref);
    match choose_input(preferred.as_ref(), &available, default.as_ref()) {
        InputChoice::Chosen(device) => Some(device.name),
        InputChoice::Substituted { used, .. } => Some(used.name),
        InputChoice::SystemDefault(device) => Some(device.name),
        InputChoice::Nothing { .. } => None,
    }
}

/// What the host app is told when the Mac's audio hardware changes.
///
/// One test, because the listener it installs is a process-wide static: the
/// host installs it once at startup and there is nothing for a second one to
/// mean.
#[cfg(test)]
mod hardware_change_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// The defect this is here for: CoreAudio was already telling Juno that a
    /// device had come or gone, and the news reached the warm microphone and
    /// stopped there. Nothing reached a window, so a mic plugged in with
    /// Settings open was absent from the list until the window was reopened.
    #[test]
    fn a_hardware_change_reaches_the_host() {
        let heard = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&heard);
        super::on_hardware_change(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        super::report_hardware_change();
        assert_eq!(heard.load(Ordering::SeqCst), 1);

        // A second install is refused rather than stacked or swapped: two
        // listeners on one change would send the list twice, and replacing the
        // first would silently drop whoever was already drawing it.
        let ignored = Arc::new(AtomicUsize::new(0));
        let other = Arc::clone(&ignored);
        super::on_hardware_change(move || {
            other.fetch_add(1, Ordering::SeqCst);
        });

        super::report_hardware_change();
        assert_eq!(heard.load(Ordering::SeqCst), 2);
        assert_eq!(ignored.load(Ordering::SeqCst), 0);
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
    use crate::device_identity::{DeviceChoice, DeviceRef};

    fn dev(id: &str, name: &str) -> DeviceRef {
        DeviceRef {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    fn devices() -> Vec<DeviceRef> {
        vec![
            dev("uid-builtin", "MacBook Air Microphone"),
            dev("uid-shure", "Shure MV7"),
        ]
    }

    fn picked(uid: &str, name: &str) -> DeviceChoice {
        DeviceChoice {
            uid: Some(uid.to_string()),
            name: name.to_string(),
        }
    }

    #[test]
    fn no_preference_follows_the_system() {
        let all = devices();
        assert_eq!(
            choose_input(None, &all, Some(&all[0])),
            InputChoice::SystemDefault(all[0].clone())
        );
    }

    #[test]
    fn a_connected_choice_is_honoured() {
        let all = devices();
        assert_eq!(
            choose_input(Some(&picked("uid-shure", "Shure MV7")), &all, Some(&all[0])),
            InputChoice::Chosen(all[1].clone())
        );
    }

    /// The choice follows the UID, so a renamed device is still the one picked.
    #[test]
    fn a_renamed_device_is_still_the_choice() {
        let all = devices();
        assert_eq!(
            choose_input(Some(&picked("uid-shure", "Old Name")), &all, Some(&all[0])),
            InputChoice::Chosen(all[1].clone())
        );
    }

    /// Two microphones with one name: the UID says which.
    #[test]
    fn two_devices_sharing_a_name_resolve_by_uid() {
        let twins = vec![
            dev("uid-usb-1", "USB Microphone"),
            dev("uid-usb-2", "USB Microphone"),
        ];
        for index in 0..2 {
            let choice = picked(&twins[index].id, "USB Microphone");
            assert_eq!(
                choose_input(Some(&choice), &twins, Some(&twins[0])),
                InputChoice::Chosen(twins[index].clone())
            );
        }
    }

    /// A device with the right name but another UID is not the choice.
    #[test]
    fn a_matching_name_is_not_enough_once_a_uid_is_stored() {
        let all = devices();
        assert_eq!(
            choose_input(Some(&picked("uid-gone", "Shure MV7")), &all, Some(&all[0])),
            InputChoice::Substituted {
                requested: "Shure MV7".to_string(),
                used: all[0].clone(),
            }
        );
    }

    /// A choice saved by an older build has a name and no UID.
    #[test]
    fn a_legacy_name_choice_still_resolves() {
        let all = devices();
        let legacy = DeviceChoice {
            uid: None,
            name: "Shure MV7".to_string(),
        };
        assert_eq!(
            choose_input(Some(&legacy), &all, Some(&all[0])),
            InputChoice::Chosen(all[1].clone())
        );
    }

    /// Unplugging the headphones must not brick dictation.
    #[test]
    fn a_vanished_choice_falls_back_to_the_system_default() {
        let all = devices();
        assert_eq!(
            choose_input(
                Some(&picked("uid-airpods", "AirPods Pro")),
                &all,
                Some(&all[0])
            ),
            InputChoice::Substituted {
                requested: "AirPods Pro".to_string(),
                used: all[0].clone(),
            }
        );
    }

    #[test]
    fn a_vanished_choice_with_nothing_left_is_a_failure() {
        assert_eq!(
            choose_input(Some(&picked("uid-airpods", "AirPods Pro")), &[], None),
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

    /// A stored blank is the same as no preference, not a device named "".
    /// Settings files written by hand have both.
    #[test]
    fn a_blank_preference_is_no_preference() {
        let all = devices();
        for blank in ["", "   "] {
            let choice = DeviceChoice {
                uid: None,
                name: blank.to_string(),
            };
            assert_eq!(
                choose_input(Some(&choice), &all, Some(&all[1])),
                InputChoice::SystemDefault(all[1].clone()),
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

#[cfg(test)]
mod freshness_tests {
    use super::Freshness;

    #[test]
    fn nothing_is_trusted_before_the_first_pass() {
        assert!(!Freshness::new().is_current());
    }

    #[test]
    fn a_pass_with_no_change_since_makes_the_stream_current() {
        let f = Freshness::new();
        let g = f.generation();
        f.verified_at(g);
        assert!(f.is_current());
    }

    #[test]
    fn a_change_makes_a_verified_stream_stale() {
        let f = Freshness::new();
        f.verified_at(f.generation());
        f.changed();
        assert!(!f.is_current(), "a press after a change must check");
    }

    /// The ordering the generation exists for: a change that lands while the
    /// warmer is checking is newer than what that pass records.
    #[test]
    fn a_change_during_a_pass_keeps_the_stream_stale() {
        let f = Freshness::new();
        let g = f.generation();
        f.changed();
        f.verified_at(g);
        assert!(!f.is_current());
        // The pass that change asked for catches up.
        f.verified_at(f.generation());
        assert!(f.is_current());
    }

    #[test]
    fn a_slow_pass_finishing_late_does_not_undo_a_newer_one() {
        let f = Freshness::new();
        let slow = f.generation();
        f.changed();
        let fast = f.generation();
        f.verified_at(fast);
        f.verified_at(slow);
        assert!(f.is_current());
        f.changed();
        assert!(!f.is_current());
    }
}
