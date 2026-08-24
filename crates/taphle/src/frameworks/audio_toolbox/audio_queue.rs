/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `AudioQueue.h` (Audio Queue Services)
//!
//! The audio playback here is mapped onto OpenAL Soft for convenience.
//! Apple's implementation probably uses Core Audio instead.

use crate::abi::{CallFromHost, GuestFunction};
use crate::audio::openal as al;
use crate::audio::openal::al_types::*;
use crate::audio::openal::{OpenAL, OpenALManager};
use crate::audio::{decode_ima4, StreamingMp3Decoder};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::OSStatus;
use crate::frameworks::core_audio_types::{
    debug_fourcc, fourcc, kAudioFormatAppleIMA4, kAudioFormatFlagIsBigEndian,
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked, kAudioFormatFlagIsSignedInteger,
    kAudioFormatLinearPCM, kAudioFormatMPEGLayer3, kAudioTimeStampSampleTimeValid,
    AudioStreamBasicDescription, AudioStreamPacketDescription, AudioTimeStamp,
};
use crate::frameworks::core_foundation::cf_run_loop::{
    kCFRunLoopCommonModes, CFRunLoopGetMain, CFRunLoopMode, CFRunLoopRef,
};
use crate::frameworks::foundation::ns_run_loop;
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::mem::{
    guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, MutVoidPtr, Ptr, SafeRead,
};
use crate::objc::msg;
use crate::Environment;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct State {
    audio_queues: HashMap<AudioQueueRef, AudioQueueHostObject>,
}
impl State {
    fn get(framework_state: &mut crate::frameworks::State) -> &mut Self {
        &mut framework_state.audio_toolbox.audio_queue
    }
    fn get_with_context<'s, 'm: 's>(
        framework_state: &'s mut crate::frameworks::State,
        manager: &'m mut OpenALManager,
    ) -> (&'s mut Self, OpenAL<'s>) {
        (
            &mut framework_state.audio_toolbox.audio_queue,
            framework_state
                .audio_toolbox
                .al_context
                .make_al_context_current(manager),
        )
    }
}

struct AudioQueueHostObject {
    format: AudioStreamBasicDescription,
    callback_proc: AudioQueueOutputCallback,
    callback_user_data: MutVoidPtr,
    /// Weak reference
    run_loop: CFRunLoopRef,
    volume: f32,
    buffers: Vec<AudioQueueBufferRef>,
    /// There is also a queue of OpenAL buffers, which must be kept in sync:
    /// the nth item in this queue must also be the nth item in the OpenAL
    /// queue, though the OpenAL queue may be shorter.
    buffer_queue: VecDeque<QueuedAudioBuffer>,
    mp3_decoder: Option<StreamingMp3Decoder>,
    is_running: AudioQueueIsRunning,
    al_source: Option<ALuint>,
    al_unused_buffers: Vec<ALuint>,
    aq_is_running_proc: Option<AudioQueuePropertyListenerProc>,
    aq_is_running_user_data: Option<MutVoidPtr>,
    is_running_handler: bool,
    /// Sample frames belonging to buffers that OpenAL has finished with and
    /// that have been unqueued from the source. This is the fixed part of the
    /// queue's playback clock; see [AudioQueueGetCurrentTime].
    frames_dequeued: f64,
    /// Sample frames of everything ever enqueued on this queue. On the same
    /// timeline as `frames_dequeued`, and always ahead of it, so the difference
    /// is what is still waiting to be played. This is what lets
    /// [AudioQueueEnqueueBufferWithParameters] say when a buffer will start.
    frames_enqueued: f64,
}

struct QueuedAudioBuffer {
    buffer_ref: AudioQueueBufferRef,
    packet_descriptions: Vec<HostPacketDescription>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HostPacketDescription {
    start_offset: usize,
    variable_frames_in_packet: u32,
    data_byte_size: usize,
}

/// Track whether the audio queue is meant to be running, in order to handle
/// OpenAL stop events caused by running out of data:
/// - If it's running, the OpenAL source can be restarted.
/// - If it's stopping asynchronously, the audio queue stop can be completed.
#[derive(PartialEq, Eq, Clone, Copy)]
enum AudioQueueIsRunning {
    Running,
    Stopping,
    Stopped,
}

#[repr(C, packed)]
pub struct OpaqueAudioQueue {
    _filler: u8,
}
unsafe impl SafeRead for OpaqueAudioQueue {}

pub type AudioQueueRef = MutPtr<OpaqueAudioQueue>;

#[repr(C, packed)]
pub struct AudioQueueBuffer {
    audio_data_bytes_capacity: u32,
    pub audio_data: MutVoidPtr,
    pub audio_data_byte_size: u32,
    user_data: MutVoidPtr,
    packet_description_capacity: u32,
    /// Should be a `MutPtr<AudioStreamPacketDescription>`, but that's not
    /// implemented yet.
    _packet_descriptions: MutVoidPtr,
    _packet_description_count: u32,
}
unsafe impl SafeRead for AudioQueueBuffer {}

pub type AudioQueueBufferRef = MutPtr<AudioQueueBuffer>;

/// (*void)(void *in_user_data, AudioQueueRef in_aq, AudioQueueBufferRef in_buf)
pub type AudioQueueOutputCallback = GuestFunction;

type AudioQueueParameterID = u32;
pub const kAudioQueueParam_Volume: AudioQueueParameterID = 1;

type AudioQueueParameterValue = f32;

pub type AudioQueuePropertyID = u32;
pub const kAudioQueueProperty_IsRunning: AudioQueuePropertyID = fourcc(b"aqrn");

/// (*void)(void *in_user_data, AudioQueueRef in_aq, AudioQueuePropertyID in_id)
type AudioQueuePropertyListenerProc = GuestFunction;

const kAudioQueueErr_InvalidBuffer: OSStatus = -66687;
const kAudioQueueErr_InvalidPropertySize: OSStatus = -66683;
const kAudioQueueErr_InvalidParameter: OSStatus = -66682;
const kAudioQueueErr_BufferInQueue: OSStatus = -66679;
const kAudioQueueErr_CannotStart: OSStatus = -66681;
const kAudioQueueErr_InvalidRunState: OSStatus = -66678;

pub fn AudioQueueNewOutput(
    env: &mut Environment,
    in_format: ConstPtr<AudioStreamBasicDescription>,
    in_callback_proc: AudioQueueOutputCallback,
    in_user_data: MutVoidPtr,
    in_callback_run_loop: CFRunLoopRef,
    in_callback_run_loop_mode: CFRunLoopMode,
    in_flags: u32,
    out_aq: MutPtr<AudioQueueRef>,
) -> OSStatus {
    // reserved
    assert!(in_flags == 0);
    // NULL is a synonym of kCFRunLoopCommonModes here
    assert!(
        in_callback_run_loop_mode.is_null() || {
            let common_modes = get_static_str(env, kCFRunLoopCommonModes);
            msg![env; in_callback_run_loop_mode isEqual:common_modes]
        }
    );

    let in_callback_run_loop = if in_callback_run_loop.is_null() {
        // FIXME: According to the documentation, "one of the audio queue's
        // internal threads" should be used if you don't specify a run loop.
        // We should have an "internal thread" instead of using the main thread.
        CFRunLoopGetMain(env)
    } else {
        in_callback_run_loop
    };

    let mut format = env.mem.read(in_format);
    if env.bundle.bundle_identifier().starts_with("com.ea.candcra")
        && format.format_id == fourcc(b".mp3")
    {
        log!("Applying game-specific hack for C&C Red Alert: Fixing hardcoded audio format from .mp3 to PCM.");
        format = AudioStreamBasicDescription {
            sample_rate: 44100.0,
            format_id: kAudioFormatLinearPCM,
            format_flags: 12,
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels_per_frame: 2,
            bits_per_channel: 16,
            _reserved: 0,
        }
    }

    let host_object = AudioQueueHostObject {
        format,
        callback_proc: in_callback_proc,
        callback_user_data: in_user_data,
        run_loop: in_callback_run_loop,
        volume: 1.0,
        buffers: Vec::new(),
        buffer_queue: VecDeque::new(),
        mp3_decoder: None,
        is_running: AudioQueueIsRunning::Stopped,
        al_source: None,
        al_unused_buffers: Vec::new(),
        aq_is_running_proc: None,
        aq_is_running_user_data: None,
        is_running_handler: false,
        frames_dequeued: 0.0,
        frames_enqueued: 0.0,
    };

    let aq_ref = env.mem.alloc_and_write(OpaqueAudioQueue { _filler: 0 });
    State::get(&mut env.framework_state)
        .audio_queues
        .insert(aq_ref, host_object);
    env.mem.write(out_aq, aq_ref);

    ns_run_loop::add_audio_queue(env, in_callback_run_loop, aq_ref);

    log_if_broken_audio_format(&format);

    if !is_supported_audio_queue_format(&format) {
        log!("Warning: Audio queue {:?} will be ignored because its format is not yet supported: {:#?}", aq_ref, format);
    }

    log_dbg!(
        "AudioQueueNewOutput() for format {:#?}, new audio queue handle: {:?}",
        format,
        aq_ref,
    );

    0 // success
}

pub fn AudioQueueGetParameter(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_param_id: AudioQueueParameterID,
    out_value: MutPtr<AudioQueueParameterValue>,
) -> OSStatus {
    return_if_null!(in_aq);

    assert!(in_param_id == kAudioQueueParam_Volume); // others unimplemented

    let state = State::get(&mut env.framework_state);
    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    env.mem.write(out_value, host_object.volume);

    0 // success
}

pub fn AudioQueueSetParameter(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_param_id: AudioQueueParameterID,
    in_value: AudioQueueParameterValue,
) -> OSStatus {
    return_if_null!(in_aq);

    assert!(in_param_id == kAudioQueueParam_Volume); // others unimplemented

    let state = State::get(&mut env.framework_state);
    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    host_object.volume = in_value;
    log_dbg!(
        "AudioQueueSetParameter kAudioQueueParam_Volume is set to {}",
        host_object.volume
    );
    if let Some(al_source) = host_object.al_source {
        let context = env
            .framework_state
            .audio_toolbox
            .make_al_context_current(&mut env.openal_manager);

        // If not clamped, OpenAL generates an error.
        // While Apple's docs states that this range is expected,
        // setting outside of range values do not generate errors
        // (tested on both macOS and iOS).
        let in_value = in_value.clamp(0.0, 1.0);

        unsafe {
            context.Sourcef(al_source, al::AL_MAX_GAIN, in_value);
            assert!(context.GetError() == 0);
        }
    }

    0 // success
}

fn AudioQueueAllocateBufferWithPacketDescriptions(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_buffer_byte_size: GuestUSize,
    _in_number_packet_desc: GuestUSize,
    out_buffer: MutPtr<AudioQueueBufferRef>,
) -> OSStatus {
    // TODO: support packet descriptions embedded in an AudioQueueBuffer. MP3
    // queues using descriptions passed directly to EnqueueBuffer are handled.
    AudioQueueAllocateBuffer(env, in_aq, in_buffer_byte_size, out_buffer)
}

pub fn AudioQueueAllocateBuffer(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_buffer_byte_size: GuestUSize,
    out_buffer: MutPtr<AudioQueueBufferRef>,
) -> OSStatus {
    return_if_null!(in_aq);

    let host_object = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap();

    let packet_description_capacity = if env
        .bundle
        .bundle_identifier()
        .starts_with("com.ea.candcra")
    {
        log!("Applying game-specific hack for C&C Red Alert: Setting packet description capacity to 1024.");
        1024
    } else {
        0
    };

    let audio_data = env.mem.alloc(in_buffer_byte_size);
    let buffer_ptr = env.mem.alloc_and_write(AudioQueueBuffer {
        audio_data_bytes_capacity: in_buffer_byte_size,
        audio_data,
        audio_data_byte_size: 0,
        user_data: Ptr::null(),
        packet_description_capacity,
        _packet_descriptions: Ptr::null(),
        _packet_description_count: 0,
    });
    host_object.buffers.push(buffer_ptr);
    env.mem.write(out_buffer, buffer_ptr);

    0 // success
}

fn AudioQueueEnqueueBufferWithParameters(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_buffer: AudioQueueBufferRef,
    in_num_packet_descs: u32,
    in_packet_descs: MutVoidPtr,
    in_trim_frames_at_start: u32,
    in_trim_frames_at_end: u32,
    in_num_param_values: u32,
    in_param_values: MutVoidPtr,
    in_start_time: ConstPtr<AudioTimeStamp>,
    out_actual_start_time: MutPtr<AudioTimeStamp>,
) -> OSStatus {
    // TODO
    assert_eq!(in_trim_frames_at_start, 0);
    assert_eq!(in_trim_frames_at_end, 0);
    assert_eq!(in_num_param_values, 0);
    assert!(in_param_values.is_null());
    assert!(in_start_time.is_null());

    // The buffer starts where the queue's enqueued audio currently ends, which
    // is what a null `in_start_time` asks for: play it after everything already
    // enqueued. Read before the enqueue below, so it does not include this
    // buffer itself.
    let start_sample_time = State::get(&mut env.framework_state)
        .audio_queues
        .get(&in_aq)
        .map_or(0.0, |host_object| host_object.frames_enqueued);

    let result =
        AudioQueueEnqueueBuffer(env, in_aq, in_buffer, in_num_packet_descs, in_packet_descs);

    if result == 0 && !out_actual_start_time.is_null() {
        let mut time_stamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
        time_stamp.sample_time = start_sample_time;
        time_stamp.flags = kAudioTimeStampSampleTimeValid;
        env.mem.write(out_actual_start_time, time_stamp);
    }

    result
}

/// Sample frames in a buffer being enqueued, counted the way the queue's own
/// format describes it.
///
/// MP3 is counted from the packet descriptions, because the buffer holds
/// encoded bytes whose frame count its byte size does not reveal. Everything
/// else is fixed-size PCM, where the byte size does.
fn enqueued_frame_count(
    mem: &Mem,
    host_object: &AudioQueueHostObject,
    buffer_ref: AudioQueueBufferRef,
    packet_descriptions: &[HostPacketDescription],
) -> f64 {
    let audio_data_byte_size = mem.read(buffer_ref).audio_data_byte_size;
    if host_object.format.format_id == kAudioFormatMPEGLayer3 {
        return f64::from(host_object.format.frames_per_packet) * packet_descriptions.len() as f64;
    }
    if host_object.format.bytes_per_frame == 0 {
        return 0.0;
    }
    f64::from(audio_data_byte_size / host_object.format.bytes_per_frame)
}

fn snapshot_packet_descriptions(
    mem: &Mem,
    audio_data_byte_size: u32,
    count: u32,
    descriptions: MutVoidPtr,
) -> Result<Vec<HostPacketDescription>, ()> {
    if count == 0 || descriptions.is_null() || count > audio_data_byte_size {
        return Err(());
    }

    let descriptions: MutPtr<AudioStreamPacketDescription> = descriptions.cast();
    let buffer_size: usize = audio_data_byte_size.try_into().map_err(|_| ())?;
    validate_packet_descriptions(
        buffer_size,
        (0..count).map(|index| mem.read(descriptions + index)),
    )
}

fn validate_packet_descriptions(
    buffer_size: usize,
    descriptions: impl IntoIterator<Item = AudioStreamPacketDescription>,
) -> Result<Vec<HostPacketDescription>, ()> {
    let mut previous_end = 0;
    let mut snapshot = Vec::new();

    for description in descriptions {
        let AudioStreamPacketDescription {
            start_offset,
            variable_frames_in_packet,
            data_byte_size,
        } = description;
        let start_offset: usize = start_offset.try_into().map_err(|_| ())?;
        let data_byte_size: usize = data_byte_size.try_into().map_err(|_| ())?;
        let end = start_offset.checked_add(data_byte_size).ok_or(())?;

        if data_byte_size == 0 || start_offset < previous_end || end > buffer_size {
            return Err(());
        }

        snapshot.push(HostPacketDescription {
            start_offset,
            variable_frames_in_packet,
            data_byte_size,
        });
        previous_end = end;
    }

    if snapshot.is_empty() {
        Err(())
    } else {
        Ok(snapshot)
    }
}

pub fn AudioQueueEnqueueBuffer(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_buffer: AudioQueueBufferRef,
    in_num_packet_descs: u32,
    in_packet_descs: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_aq);

    let host_object = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap();

    if !host_object.buffers.contains(&in_buffer) {
        return kAudioQueueErr_InvalidBuffer;
    }

    let packet_descriptions = if host_object.format.format_id == kAudioFormatMPEGLayer3 {
        let buffer = env.mem.read(in_buffer);
        let Ok(packet_descriptions) = snapshot_packet_descriptions(
            &env.mem,
            buffer.audio_data_byte_size,
            in_num_packet_descs,
            in_packet_descs,
        ) else {
            log!(
                "Warning: Invalid or missing direct MP3 packet descriptions for audio queue {:?}",
                in_aq
            );
            return kAudioQueueErr_InvalidParameter;
        };
        packet_descriptions
    } else {
        // Some fixed-size formats supply a redundant description. Keep
        // accepting that behavior as before.
        Vec::new()
    };

    let buffer_frames =
        enqueued_frame_count(&env.mem, host_object, in_buffer, &packet_descriptions);
    host_object.frames_enqueued += buffer_frames;

    host_object.buffer_queue.push_back(QueuedAudioBuffer {
        buffer_ref: in_buffer,
        packet_descriptions,
    });
    log_dbg!("New buffer enqueued: {:?}", in_buffer);

    0 // success
}

fn AudioQueueAddPropertyListener(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_id: AudioQueuePropertyID,
    in_proc: AudioQueuePropertyListenerProc,
    in_user_data: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_aq);

    if in_id == kAudioQueueProperty_IsRunning {
        let host_object = State::get(&mut env.framework_state)
            .audio_queues
            .get_mut(&in_aq)
            .unwrap();

        host_object.aq_is_running_proc = Some(in_proc);
        host_object.aq_is_running_user_data = Some(in_user_data);
    } else {
        log!(
            "TODO: AudioQueueAddPropertyListener({:?}, {}, {:?}, {:?})",
            in_aq,
            debug_fourcc(in_id),
            in_proc,
            in_user_data
        );
    }
    0 // success
}
fn AudioQueueRemovePropertyListener(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_id: AudioQueuePropertyID,
    in_proc: AudioQueuePropertyListenerProc,
    in_user_data: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_aq);

    if in_id == kAudioQueueProperty_IsRunning {
        let host_object = State::get(&mut env.framework_state)
            .audio_queues
            .get_mut(&in_aq)
            .unwrap();

        host_object.aq_is_running_proc = None;
        host_object.aq_is_running_user_data = None;
    } else {
        log!(
            "TODO: AudioQueueRemovePropertyListener({:?}, {}, {:?}, {:?})",
            in_aq,
            debug_fourcc(in_id),
            in_proc,
            in_user_data
        );
    }
    0 // success
}

fn property_size(property_id: AudioQueuePropertyID) -> GuestUSize {
    match property_id {
        kAudioQueueProperty_IsRunning => guest_size_of::<u32>(),
        _ => unimplemented!("Unimplemented property ID: {}", debug_fourcc(property_id)),
    }
}

fn AudioQueueGetPropertySize(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_property_id: AudioQueuePropertyID,
    out_data_size: MutPtr<u32>,
) -> OSStatus {
    return_if_null!(in_aq);

    env.mem.write(out_data_size, property_size(in_property_id));
    0 // success
}

fn AudioQueueGetProperty(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_property_id: AudioQueuePropertyID,
    out_property_data: MutVoidPtr,
    io_data_size: MutPtr<u32>,
) -> OSStatus {
    return_if_null!(in_aq);

    let required_size = property_size(in_property_id);
    if env.mem.read(io_data_size) != required_size {
        log!("Warning: AudioQueueGetProperty() failed");
        return kAudioQueueErr_InvalidPropertySize;
    }

    let host_object = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap();

    match in_property_id {
        kAudioQueueProperty_IsRunning => {
            let is_running: u32 = match host_object.is_running {
                AudioQueueIsRunning::Running => 1,
                AudioQueueIsRunning::Stopping => 1,
                AudioQueueIsRunning::Stopped => 0,
            };
            env.mem.write(out_property_data.cast(), is_running);
        }
        _ => unreachable!(),
    }

    0 // success
}

fn AudioQueueSetProperty(
    _env: &mut Environment,
    in_aq: AudioQueueRef,
    in_property_id: AudioQueuePropertyID,
    in_data: ConstVoidPtr,
    in_data_size: u32,
) -> OSStatus {
    return_if_null!(in_aq);

    // Settable audio-queue properties are configuration hints — level metering,
    // the format magic cookie, channel assignments, codec policy — that
    // tapHLE's mixer does not need. Accept and ignore them rather than failing
    // the call; an app that cannot set an optional property may take an error
    // path it does not need to.
    log_dbg!(
        "TODO: ignoring AudioQueueSetProperty({:?}, {}, {:?}, {:#x})",
        in_aq,
        debug_fourcc(in_property_id),
        in_data,
        in_data_size,
    );
    0 // success
}

pub fn log_if_broken_audio_format(format: &AudioStreamBasicDescription) {
    let bytes_per_channel = format.bits_per_channel / 8;
    let expected_bytes_per_packet = format.bytes_per_frame * format.frames_per_packet;
    let expected_bytes_per_frame = format.channels_per_frame * bytes_per_channel;
    if format.bytes_per_packet < expected_bytes_per_packet
        || format.bytes_per_frame < expected_bytes_per_frame
    {
        log!(
            "Warning: Stream format has non-sensical values: {:?}",
            format
        );
    }
}

/// Check if the format of an audio queue is one we currently support.
/// If not, we should skip trying to play it rather than crash.
pub fn is_supported_audio_format(format: &AudioStreamBasicDescription) -> bool {
    let &AudioStreamBasicDescription {
        format_id,
        format_flags,
        channels_per_frame,
        bits_per_channel,
        bytes_per_frame,
        ..
    } = format;
    match format_id {
        kAudioFormatAppleIMA4 => (channels_per_frame == 1) || (channels_per_frame == 2),
        kAudioFormatLinearPCM => {
            // TODO: support more PCM formats
            (channels_per_frame == 1 || channels_per_frame == 2)
                && (bits_per_channel == 8 || bits_per_channel == 16 || bits_per_channel == 32)
                && ((format_flags & kAudioFormatFlagIsPacked) != 0
                    || ((bits_per_channel / 8) * channels_per_frame) == bytes_per_frame)
                && (format_flags & kAudioFormatFlagIsBigEndian) == 0
                && (format_flags & kAudioFormatFlagIsFloat) == 0
        }
        _ => false,
    }
}

fn is_supported_audio_queue_format(format: &AudioStreamBasicDescription) -> bool {
    if is_supported_audio_format(format) {
        return true;
    }

    let &AudioStreamBasicDescription {
        sample_rate,
        format_id,
        format_flags,
        bytes_per_packet,
        frames_per_packet,
        bytes_per_frame,
        channels_per_frame,
        bits_per_channel,
        ..
    } = format;
    if format_id != kAudioFormatMPEGLayer3 {
        return false;
    }

    let rate_and_frame_size_match = match sample_rate as u32 {
        32_000 | 44_100 | 48_000 => frames_per_packet == 1152,
        8_000 | 11_025 | 12_000 | 16_000 | 22_050 | 24_000 => frames_per_packet == 576,
        _ => false,
    };
    sample_rate.is_finite()
        && sample_rate == f64::from(sample_rate as u32)
        && rate_and_frame_size_match
        && format_flags == 0
        && bytes_per_packet == 0
        && bytes_per_frame == 0
        && matches!(channels_per_frame, 1 | 2)
        && bits_per_channel == 0
}

/// Decode an [AudioQueueBuffer] or [super::audio_unit::AudioBuffer]'s content
/// to raw PCM suitable for an OpenAL buffer.
pub fn decode_buffer(
    mem: &Mem,
    format: &AudioStreamBasicDescription,
    audio_data: MutPtr<u8>,
    audio_data_byte_size: GuestUSize,
) -> (ALenum, ALsizei, Vec<u8>) {
    let data_slice = mem.bytes_at(audio_data, audio_data_byte_size);

    assert!(is_supported_audio_format(format));

    match format.format_id {
        kAudioFormatAppleIMA4 => {
            assert!(data_slice.len().is_multiple_of(34));
            let mut out_pcm = Vec::<u8>::with_capacity((data_slice.len() / 34) * 64 * 2);
            let packets = data_slice.chunks(34);

            if format.channels_per_frame == 1 {
                for packet in packets {
                    let pcm_packet: [i16; 64] = decode_ima4(packet.try_into().unwrap());
                    let pcm_bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(pcm_packet.as_ptr() as *const u8, 128)
                    };
                    out_pcm.extend_from_slice(pcm_bytes);
                }

                (al::AL_FORMAT_MONO16, format.sample_rate as ALsizei, out_pcm)
            } else {
                let mut peekable_packets = packets.peekable();
                while peekable_packets.peek().is_some() {
                    let left = peekable_packets.next().unwrap();
                    let left_pcm_packet: [i16; 64] = decode_ima4(left.try_into().unwrap());
                    let right = peekable_packets.next().unwrap();
                    let right_pcm_packet: [i16; 64] = decode_ima4(right.try_into().unwrap());
                    for (l, r) in left_pcm_packet.iter().zip(right_pcm_packet.iter()) {
                        out_pcm.extend_from_slice(&l.to_le_bytes());
                        out_pcm.extend_from_slice(&r.to_le_bytes());
                    }
                }

                (
                    al::AL_FORMAT_STEREO16,
                    format.sample_rate as ALsizei,
                    out_pcm,
                )
            }
        }
        kAudioFormatLinearPCM => {
            // The end of the data might be misaligned (this happens in Crash
            // Bandicoot Nitro Kart 3D somehow).
            let misaligned_by = data_slice.len() % (format.bytes_per_frame as usize);
            let data_slice = if misaligned_by != 0 {
                &data_slice[..data_slice.len() - misaligned_by]
            } else {
                data_slice
            };

            let bytes_per_channel = format.bits_per_channel / 8;
            let actual_bytes_per_frame = format.channels_per_frame * bytes_per_channel;
            let actual_channels_per_frame = format.bytes_per_frame / bytes_per_channel;

            // In case the audio format has inconsistent values, we apply some
            // processing before passing it to OpenAL.
            // This is the case in Resident Evil 4
            let processed_data: Vec<u8> = if actual_bytes_per_frame == format.bytes_per_frame {
                data_slice.to_owned()
            } else {
                let actual_frame_count = data_slice.len() / actual_bytes_per_frame as usize;
                let processed_frame_count = format.bytes_per_frame as usize * actual_frame_count;
                let mut processed_data = Vec::<u8>::with_capacity(processed_frame_count);
                for frame in data_slice.chunks(actual_bytes_per_frame as usize) {
                    // Fetch only frame bytes
                    let frame_bytes = &frame[frame.len() - format.bytes_per_frame as usize..];
                    // Change from big to little endian
                    // It's been observed in Resident Evil 4 that, although the
                    // audio format doesn't say anything about it being in big
                    // endian, the data in the buffer has their values in big
                    // endian and must be converted to little endian before
                    // passing them to OpenAL.
                    match format.bytes_per_frame {
                        1 => processed_data.extend(
                            &u8::from_be_bytes(frame_bytes.try_into().unwrap()).to_le_bytes(),
                        ),
                        2 => processed_data.extend_from_slice(
                            &u16::from_be_bytes(frame_bytes.try_into().unwrap()).to_le_bytes(),
                        ),
                        4 => processed_data.extend_from_slice(
                            &u32::from_be_bytes(frame_bytes.try_into().unwrap()).to_le_bytes(),
                        ),
                        8 => processed_data.extend_from_slice(
                            &u64::from_be_bytes(frame_bytes.try_into().unwrap()).to_le_bytes(),
                        ),
                        16 => processed_data.extend_from_slice(
                            &u128::from_be_bytes(frame_bytes.try_into().unwrap()).to_le_bytes(),
                        ),
                        _ => unimplemented!(),
                    };
                }
                processed_data
            };

            let f = match (actual_channels_per_frame, format.bits_per_channel) {
                (1, 8) => al::AL_FORMAT_MONO8,
                (1, 16) => al::AL_FORMAT_MONO16,
                (2, 8) => al::AL_FORMAT_STEREO8,
                (2, 16) => al::AL_FORMAT_STEREO16,
                (2, 32) => {
                    assert!((format.format_flags & kAudioFormatFlagIsSignedInteger) != 0);
                    assert!(processed_data.len().is_multiple_of(4));
                    let new_size = (processed_data.len() / 4) * 2; // size from 32-bit to 16-bit
                    let mut new_processed_data = Vec::<u8>::with_capacity(new_size);
                    for chunk in processed_data.chunks(4) {
                        let val: i32 = i32::from_le_bytes(chunk.try_into().unwrap());
                        let new_val: i16 = (val >> 16) as i16;
                        new_processed_data.extend(new_val.to_le_bytes());
                    }
                    return (
                        al::AL_FORMAT_STEREO16,
                        format.sample_rate as ALsizei,
                        new_processed_data,
                    );
                }
                _ => unreachable!(),
            };
            (f, format.sample_rate as ALsizei, processed_data)
        }
        _ => unreachable!(),
    }
}

fn decode_mp3_packets(
    mem: &Mem,
    format: &AudioStreamBasicDescription,
    audio_data: MutPtr<u8>,
    audio_data_byte_size: GuestUSize,
    packet_descriptions: &[HostPacketDescription],
    decoder: &mut StreamingMp3Decoder,
) -> Result<(ALenum, ALsizei, Vec<u8>), ()> {
    let &AudioStreamBasicDescription {
        sample_rate,
        frames_per_packet,
        channels_per_frame,
        ..
    } = format;
    let encoded = mem.bytes_at(audio_data, audio_data_byte_size);
    let mut pcm = Vec::new();
    let mut decoded_packet_count = 0;

    for &HostPacketDescription {
        start_offset,
        variable_frames_in_packet,
        data_byte_size,
    } in packet_descriptions
    {
        let end = start_offset + data_byte_size;
        let frame_count = if variable_frames_in_packet == 0 {
            frames_per_packet
        } else {
            variable_frames_in_packet
        };
        let Some(encoded_packet) = encoded.get(start_offset..end) else {
            log!("Warning: MP3 packet description changed after enqueue");
            continue;
        };
        match decoder.decode_packet(encoded_packet, frame_count) {
            Ok(decoded) => {
                pcm.extend_from_slice(&decoded);
                decoded_packet_count += 1;
            }
            Err(error) => {
                // Symphonia documents packet decode errors as recoverable: the
                // bad packet should be discarded and decoding may continue.
                log!("Warning: Discarding an invalid MP3 packet: {}", error);
            }
        }
    }

    if decoded_packet_count == 0 {
        return Err(());
    }

    let al_format = match channels_per_frame {
        1 => al::AL_FORMAT_MONO16,
        2 => al::AL_FORMAT_STEREO16,
        _ => unreachable!(),
    };
    Ok((al_format, sample_rate as ALsizei, pcm))
}

fn audio_queue_buffer_frame_count(
    format: &AudioStreamBasicDescription,
    audio_data_byte_size: u32,
    packet_descriptions: &[HostPacketDescription],
) -> u32 {
    let &AudioStreamBasicDescription {
        format_id,
        bytes_per_packet,
        frames_per_packet,
        bytes_per_frame,
        ..
    } = format;
    if format_id == kAudioFormatMPEGLayer3 {
        return packet_descriptions.iter().fold(0, |total, description| {
            let frames = if description.variable_frames_in_packet == 0 {
                frames_per_packet
            } else {
                description.variable_frames_in_packet
            };
            total.saturating_add(frames)
        });
    }

    if let Some(packet_count) = audio_data_byte_size.checked_div(bytes_per_packet) {
        return packet_count.saturating_mul(frames_per_packet);
    }
    if let Some(frame_count) = audio_data_byte_size.checked_div(bytes_per_frame) {
        return frame_count;
    }
    0
}

/// Ensure an audio queue has an OpenAL source and at least one queued OpenAL
/// buffer.
///
/// `frames_to_prepare` is `None` for normal streaming, `Some(0)` to prepare
/// every enqueued buffer, or `Some(n)` to prepare at least `n` frames.
fn prime_audio_queue(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    frames_to_prepare: Option<u32>,
) -> Result<u32, ()> {
    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    if !is_supported_audio_queue_format(&host_object.format) {
        return Err(());
    }

    if host_object.format.format_id == kAudioFormatMPEGLayer3 && host_object.mp3_decoder.is_none() {
        let sample_rate = host_object.format.sample_rate as u32;
        let channels = host_object.format.channels_per_frame;
        host_object.mp3_decoder = Some(StreamingMp3Decoder::new(sample_rate, channels).map_err(
            |error| {
                log!("Warning: Could not initialize MP3 audio queue: {}", error);
            },
        )?);
    }

    if host_object.al_source.is_none() {
        // If not clamped, OpenAL generates an error.
        // While Apple's docs states that this range is expected,
        // setting outside of range values do not generate errors
        // (tested on both macOS and iOS).
        let volume = host_object.volume.clamp(0.0, 1.0);
        let mut al_source = 0;
        unsafe {
            context.GenSources(1, &mut al_source);
            context.Sourcef(al_source, al::AL_MAX_GAIN, volume);
            assert!(context.GetError() == 0);
        };
        host_object.al_source = Some(al_source);
    }
    let al_source = host_object.al_source.unwrap();
    let mut prepared_frames = 0u32;

    loop {
        let mut al_buffers_queued = 0;
        let mut al_buffers_processed = 0;
        unsafe {
            context.GetSourcei(al_source, al::AL_BUFFERS_QUEUED, &mut al_buffers_queued);
            context.GetSourcei(
                al_source,
                al::AL_BUFFERS_PROCESSED,
                &mut al_buffers_processed,
            );
            assert!(context.GetError() == 0);
        }
        let al_buffers_queued: usize = al_buffers_queued.try_into().unwrap();
        let al_buffers_processed: usize = al_buffers_processed.try_into().unwrap();

        assert!(al_buffers_queued <= host_object.buffer_queue.len());
        let unprocessed_buffers = al_buffers_queued - al_buffers_processed;

        let requested_frames_are_ready = matches!(frames_to_prepare, Some(frame_count) if frame_count != 0 && prepared_frames >= frame_count);
        if al_buffers_queued == host_object.buffer_queue.len()
            || requested_frames_are_ready
            || (frames_to_prepare.is_none() && unprocessed_buffers > 1)
        {
            break;
        }

        let next_buffer_idx = al_buffers_queued;
        let next_buffer_ref = host_object.buffer_queue[next_buffer_idx].buffer_ref;
        let packet_descriptions = host_object.buffer_queue[next_buffer_idx]
            .packet_descriptions
            .clone();
        let next_buffer = env.mem.read(next_buffer_ref);

        log_dbg!(
            "Decoding buffer {:?} for queue {:?}",
            next_buffer_ref,
            in_aq
        );

        let (al_format, al_frequency, data) =
            if host_object.format.format_id == kAudioFormatMPEGLayer3 {
                decode_mp3_packets(
                    &env.mem,
                    &host_object.format,
                    next_buffer.audio_data.cast(),
                    next_buffer.audio_data_byte_size,
                    &packet_descriptions,
                    host_object.mp3_decoder.as_mut().unwrap(),
                )?
            } else {
                decode_buffer(
                    &env.mem,
                    &host_object.format,
                    next_buffer.audio_data.cast(),
                    next_buffer.audio_data_byte_size,
                )
            };

        let next_al_buffer = host_object.al_unused_buffers.pop().unwrap_or_else(|| {
            let mut al_buffer = 0;
            unsafe { context.GenBuffers(1, &mut al_buffer) };
            assert!(unsafe { context.GetError() } == 0);
            al_buffer
        });
        unsafe {
            context.BufferData(
                next_al_buffer,
                al_format,
                data.as_ptr() as *const ALvoid,
                data.len().try_into().unwrap(),
                al_frequency,
            )
        };
        unsafe { context.SourceQueueBuffers(al_source, 1, &next_al_buffer) };
        assert!(unsafe { context.GetError() } == 0);
        prepared_frames = prepared_frames.saturating_add(audio_queue_buffer_frame_count(
            &host_object.format,
            next_buffer.audio_data_byte_size,
            &packet_descriptions,
        ));
    }
    Ok(prepared_frames)
}

/// How many sample frames an OpenAL buffer holds, asked of OpenAL rather than
/// derived from the audio queue's format.
///
/// This matters for MP3 queues: an enqueued buffer there holds encoded bytes
/// whose frame count the format description cannot express, but by the time
/// OpenAL has the buffer it holds decoded PCM and knows its own size.
fn al_buffer_frame_count(al_buffer: ALuint, context: &OpenAL<'_>) -> f64 {
    let (mut size, mut bits, mut channels) = (0, 0, 0);
    unsafe {
        context.GetBufferi(al_buffer, al::AL_SIZE, &mut size);
        context.GetBufferi(al_buffer, al::AL_BITS, &mut bits);
        context.GetBufferi(al_buffer, al::AL_CHANNELS, &mut channels);
        assert!(context.GetError() == 0);
    }
    let bytes_per_frame = (bits / 8) * channels;
    if bytes_per_frame <= 0 {
        return 0.0;
    }
    f64::from(size / bytes_per_frame)
}

/// The queue's playback position, as a count of sample frames since it started.
///
/// This is assembled from two parts: the frames of buffers OpenAL has finished
/// and that have been unqueued, plus the source's offset into the buffer it is
/// playing now. The seam is where the accuracy limit is — a buffer that OpenAL
/// has finished but that has not been unqueued yet is counted by neither, so
/// the clock can briefly sit still and then catch up. `handle_audio_queue`
/// unqueues on every run loop turn, which keeps that window to well under one
/// buffer, but it is not the sample-exact clock the real API provides.
///
/// `in_timeline` is for detecting discontinuities across a device change or
/// hardware overload; tapHLE has neither, so no timeline can have been created
/// and none is honoured here.
fn AudioQueueGetCurrentTime(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_timeline: MutVoidPtr,
    out_time_stamp: MutPtr<AudioTimeStamp>,
    out_timeline_discontinuity: MutPtr<bool>,
) -> OSStatus {
    return_if_null!(in_aq);

    if !in_timeline.is_null() {
        log_dbg!("AudioQueueGetCurrentTime: ignoring timeline {in_timeline:?}");
    }
    if !out_timeline_discontinuity.is_null() {
        // Nothing here can produce a discontinuity, so this is not a
        // placeholder answer: it is the correct one.
        env.mem.write(out_timeline_discontinuity, false);
    }

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);
    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    let Some(al_source) = host_object.al_source else {
        // Nothing has ever been primed, so there is no timeline to report a
        // position on. This is the run state the API documents an error for.
        return kAudioQueueErr_InvalidRunState;
    };

    let mut sample_offset = 0;
    unsafe {
        context.GetSourcei(al_source, al::AL_SAMPLE_OFFSET, &mut sample_offset);
        assert!(context.GetError() == 0);
    }
    let sample_time = host_object.frames_dequeued + f64::from(sample_offset);

    if out_time_stamp.is_null() {
        return 0;
    }
    let mut time_stamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
    time_stamp.sample_time = sample_time;
    // Only the sample time is filled in, and the flags say exactly that. A
    // caller that reads mHostTime anyway gets a zero it was told not to trust.
    time_stamp.flags = kAudioTimeStampSampleTimeValid;
    env.mem.write(out_time_stamp, time_stamp);

    0 // success
}

fn unqueue_buffers<F: FnMut(ALuint)>(al_source: ALuint, context: &OpenAL<'_>, mut callback: F) {
    loop {
        let mut al_buffers_processed = 0;
        unsafe {
            context.GetSourcei(
                al_source,
                al::AL_BUFFERS_PROCESSED,
                &mut al_buffers_processed,
            );
            assert!(context.GetError() == 0);
        }
        if al_buffers_processed == 0 {
            break;
        }

        let mut al_buffer = 0;
        unsafe {
            context.SourceUnqueueBuffers(al_source, 1, &mut al_buffer);
            assert!(context.GetError() == 0);
        }

        callback(al_buffer);
    }
}

fn delete_owned_al_source(al_source: &mut Option<ALuint>, delete_source: impl FnOnce(ALuint)) {
    if let Some(al_source) = al_source.take() {
        delete_source(al_source);
    }
}

/// For use by `NSRunLoop`: check the status of an audio queue, recycle buffers,
/// call callbacks, push new buffers etc.
pub fn handle_audio_queue(env: &mut Environment, in_aq: AudioQueueRef) {
    // Collect used buffers and call the user callback so the app can provide
    // new buffers.

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();
    let Some(al_source) = host_object.al_source else {
        return;
    };
    if !is_supported_audio_queue_format(&host_object.format) {
        return;
    }
    if host_object.is_running_handler {
        // Already running, prevent infinite loop from reentrancy
        return;
    }

    host_object.is_running_handler = true;

    let mut buffers_to_reuse = Vec::new();

    unqueue_buffers(al_source, &context, |al_buffer| {
        host_object.frames_dequeued += al_buffer_frame_count(al_buffer, &context);
        host_object.al_unused_buffers.push(al_buffer);
        let queued_buffer = host_object.buffer_queue.pop_front().unwrap();
        buffers_to_reuse.push(queued_buffer.buffer_ref);
    });

    let &mut AudioQueueHostObject {
        callback_proc,
        callback_user_data,
        is_running,
        ..
    } = host_object;

    for buffer_ref in buffers_to_reuse.drain(..) {
        log_dbg!(
            "Recyling buffer {:?} for queue {:?}. Calling callback {:?} with user data {:?}.",
            buffer_ref,
            in_aq,
            callback_proc,
            callback_user_data
        );

        let () = callback_proc.call_from_host(env, (callback_user_data, in_aq, buffer_ref));
    }

    // Push new buffers etc.

    _ = prime_audio_queue(env, in_aq, None);

    let context = env
        .framework_state
        .audio_toolbox
        .make_al_context_current(&mut env.openal_manager);

    if is_running != AudioQueueIsRunning::Stopped {
        unsafe {
            let mut al_source_state = 0;
            context.GetSourcei(al_source, al::AL_SOURCE_STATE, &mut al_source_state);
            assert!(context.GetError() == 0);
            // Source probably ran out data and needs restarting
            // TODO: We currently have to do this even when tapHLE is not
            // lagging, because we're not ensuring OpenAL always has at least
            // one buffer it hasn't processed yet. We need to change our queue
            // handling.
            if al_source_state == al::AL_STOPPED {
                context.SourcePlay(al_source);
                log_dbg!("Restarted OpenAL source for queue {:?}", in_aq);
            }
        }
    }

    if is_running == AudioQueueIsRunning::Stopping {
        let mut al_source_state = 0;
        unsafe {
            context.GetSourcei(al_source, al::AL_SOURCE_STATE, &mut al_source_state);
            assert!(context.GetError() == 0);
        }

        // If OpenAL still says the source is stopped, it must have run out of
        // data, and therefore it's time to complete the "asynchronous stop".
        if al_source_state == al::AL_STOPPED {
            log_dbg!(
                "OpenAL source stopped for queue {:?}, completing asynchronous stop.",
                in_aq
            );
            finish_stopping_audio_queue(env, in_aq);
        }
    }

    if let Some(host_object) = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
    {
        host_object.is_running_handler = false;
    }
}

fn AudioQueuePrime(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_number_of_frames_to_prepare: u32,
    out_number_of_frames_prepared: MutPtr<u32>,
) -> OSStatus {
    return_if_null!(in_aq);

    match prime_audio_queue(env, in_aq, Some(in_number_of_frames_to_prepare)) {
        Ok(frames_prepared) => {
            if !out_number_of_frames_prepared.is_null() {
                env.mem
                    .write(out_number_of_frames_prepared, frames_prepared);
            }
            0 // success
        }
        Err(_) => {
            log!("Warning: Cannot prime audio queue!");
            kAudioQueueErr_CannotStart
        }
    }
}

// Offline rendering turns an audio queue inside out. Instead of the queue
// pulling buffers and handing them to the output device on its own clock, the
// caller drives it: it sets a destination format, then calls
// AudioQueueOfflineRender to synchronously convert as much audio as it asks
// for, into its own buffer, as fast as the CPU allows. Apps use it to decode
// compressed audio through the system codec rather than to play anything, and
// Unity uses it for exactly that on every compressed clip it loads.
//
// tapHLE has no such path: its queues exist to feed OpenAL. Saying so through
// the documented failure channel is what these two do. That is worth more than
// it sounds, because the alternative was aborting the emulator: a caller has
// to handle a queue that will not render offline, since the hardware codec can
// be busy with another app, so the error path already exists and is well
// trodden. This app takes it, drops the clip, and carries on.
//
// kAudio_UnimplementedError is Core Audio's own "this implementation does not
// do that", which is precisely the claim being made. Implementing offline
// rendering properly means a synchronous decode-and-convert path with sample
// rate and channel conversion, and belongs on its own branch.

/// Core Audio's generic "not implemented" status.
const kAudio_UnimplementedError: OSStatus = -4;

fn AudioQueueSetOfflineRenderFormat(
    _env: &mut Environment,
    in_aq: AudioQueueRef,
    _in_format: ConstVoidPtr, // const AudioStreamBasicDescription*
    _in_layout: ConstVoidPtr, // const AudioChannelLayout*
) -> OSStatus {
    return_if_null!(in_aq);

    log_once!(
        "TODO: AudioQueueOfflineRender is not implemented, so \
         AudioQueueSetOfflineRenderFormat fails. An app decoding audio this way \
         will lose those sounds but should keep running."
    );
    kAudio_UnimplementedError
}

fn AudioQueueOfflineRender(
    _env: &mut Environment,
    in_aq: AudioQueueRef,
    _in_timestamp: ConstVoidPtr, // const AudioTimeStamp*
    _io_buffer: AudioQueueBufferRef,
    _in_number_frames: u32,
) -> OSStatus {
    return_if_null!(in_aq);

    // Reachable only if a caller ignored the failure above, but it must not be
    // the thing that aborts the emulator either.
    log_once!("TODO: AudioQueueOfflineRender is not implemented");
    kAudio_UnimplementedError
}

fn notify_aq_is_running(env: &mut Environment, in_aq: AudioQueueRef) {
    let host_object = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap();

    if let (Some(in_proc), Some(in_user_data)) = (
        host_object.aq_is_running_proc,
        host_object.aq_is_running_user_data,
    ) {
        <GuestFunction as CallFromHost<(), (MutVoidPtr, Ptr<OpaqueAudioQueue, true>, u32)>>::
        call_from_host(
            &in_proc, env, (in_user_data, in_aq, kAudioQueueProperty_IsRunning)
        );
    }
}

pub fn AudioQueueStart(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_device_start_time: ConstPtr<AudioTimeStamp>,
) -> OSStatus {
    return_if_null!(in_aq);

    assert!(in_device_start_time.is_null()); // TODO

    _ = prime_audio_queue(env, in_aq, None);

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    host_object.is_running = AudioQueueIsRunning::Running;

    if is_supported_audio_queue_format(&host_object.format) {
        let al_source = host_object.al_source.unwrap();
        unsafe { context.SourcePlay(al_source) };
        assert!(unsafe { context.GetError() } == 0);
    } else {
        log!(
            "AudioQueueStart: Unsupported format {:?}",
            host_object.format
        );
    }

    notify_aq_is_running(env, in_aq);

    0 // success
}

pub fn AudioQueuePause(env: &mut Environment, in_aq: AudioQueueRef) -> OSStatus {
    return_if_null!(in_aq);

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();
    // FIXME: is this correct? is it notifiable?
    host_object.is_running = AudioQueueIsRunning::Stopped;
    if let Some(al_source) = host_object.al_source {
        unsafe { context.SourcePause(al_source) };
        assert!(unsafe { context.GetError() } == 0);
    }

    0 // success
}

fn finish_stopping_audio_queue(env: &mut Environment, in_aq: AudioQueueRef) {
    // OpenAL stop is not done here because it would be redundant in the case
    // of an asynchronous stop, where the audio queue stopping is triggered by
    // the OpenAL queue stopping.
    AudioQueueReset(env, in_aq);
    State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap()
        .is_running = AudioQueueIsRunning::Stopped;
    notify_aq_is_running(env, in_aq);
}

pub fn AudioQueueStop(env: &mut Environment, in_aq: AudioQueueRef, in_immediate: bool) -> OSStatus {
    return_if_null!(in_aq);

    if in_immediate {
        log_dbg!("Performing immediate AudioQueueStop for {:?}.", in_aq);

        let (state, context) =
            State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

        let host_object = state.audio_queues.get_mut(&in_aq).unwrap();
        if let Some(al_source) = host_object.al_source {
            unsafe { context.SourceStop(al_source) };
            assert!(unsafe { context.GetError() } == 0);
        };

        finish_stopping_audio_queue(env, in_aq);
    } else {
        let state = State::get(&mut env.framework_state);
        let host_object = state.audio_queues.get_mut(&in_aq).unwrap();
        if host_object.is_running != AudioQueueIsRunning::Stopped {
            log_dbg!("Starting asynchronous AudioQueueStop for {:?}.", in_aq);
            host_object.is_running = AudioQueueIsRunning::Stopping;
        } else {
            log_dbg!(
                "Ignoring asynchronous AudioQueueStop for {:?} (already stopped).",
                in_aq
            );
        }
    }

    0 // success
}

fn AudioQueueReset(env: &mut Environment, in_aq: AudioQueueRef) -> OSStatus {
    return_if_null!(in_aq);

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    log_dbg!("Resetting queue {:?}.", in_aq);

    let host_object = state.audio_queues.get_mut(&in_aq).unwrap();

    if let Some(al_source) = host_object.al_source {
        unsafe {
            let mut al_source_state = 0;
            context.GetSourcei(al_source, al::AL_SOURCE_STATE, &mut al_source_state);
            assert!(context.GetError() == 0);
            if al_source_state != al::AL_STOPPED {
                // If the source is not already stopped, it must be stopped in
                // order to be able to clear its buffer queue. Note that the
                // audio queue may still be considered "running".
                context.SourceStop(al_source);
                assert!(context.GetError() == 0);
            }
        }

        unqueue_buffers(al_source, &context, |al_buffer| {
            host_object.al_unused_buffers.push(al_buffer);
            host_object.buffer_queue.pop_front().unwrap();
        });
    }

    host_object.buffer_queue.clear();
    // The reset discards everything enqueued, so the playback clock starts
    // over. -[AudioQueueGetCurrentTime] is documented as measuring from when
    // the queue started, and after a reset that is now.
    host_object.frames_dequeued = 0.0;
    host_object.frames_enqueued = 0.0;
    if let Some(decoder) = host_object.mp3_decoder.as_mut() {
        decoder.reset();
    }

    0 // success
}

fn AudioQueueFlush(_env: &mut Environment, in_aq: AudioQueueRef) -> OSStatus {
    return_if_null!(in_aq);
    // TODO
    0 // success
}

fn AudioQueueFreeBuffer(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_buffer: AudioQueueBufferRef,
) -> OSStatus {
    return_if_null!(in_aq);

    let host_object = State::get(&mut env.framework_state)
        .audio_queues
        .get_mut(&in_aq)
        .unwrap();

    if host_object
        .buffer_queue
        .iter()
        .any(|queued_buffer| queued_buffer.buffer_ref == in_buffer)
    {
        return kAudioQueueErr_BufferInQueue;
    }

    if let Some(index) = host_object.buffers.iter().position(|x| x == &in_buffer) {
        host_object.buffers.remove(index);

        log_dbg!("Freeing buffer: {:?}", in_buffer);

        let buffer = env.mem.read(in_buffer);
        env.mem.free(buffer.audio_data);
        env.mem.free(in_buffer.cast());

        0 // success
    } else {
        kAudioQueueErr_InvalidBuffer
    }
}

pub fn AudioQueueDispose(
    env: &mut Environment,
    in_aq: AudioQueueRef,
    in_immediate: bool,
) -> OSStatus {
    return_if_null!(in_aq);

    assert!(in_immediate); // TODO

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    let mut host_object = state.audio_queues.remove(&in_aq).unwrap();
    log_dbg!("Disposing of audio queue {:?}", in_aq);

    env.mem.free(in_aq.cast());

    for buffer_ptr in host_object.buffers {
        let buffer = env.mem.read(buffer_ptr);
        env.mem.free(buffer.audio_data);
        env.mem.free(buffer_ptr.cast());
    }

    if let Some(al_source) = host_object.al_source {
        unsafe {
            context.SourceStop(al_source);
            assert!(context.GetError() == 0);
        }

        unqueue_buffers(al_source, &context, |al_buffer| {
            host_object.al_unused_buffers.push(al_buffer)
        });

        unsafe {
            context.DeleteBuffers(
                host_object.al_unused_buffers.len().try_into().unwrap(),
                host_object.al_unused_buffers.as_ptr(),
            );
            assert!(context.GetError() == 0);
        }
    }

    // AudioQueue owns the source it lazily created while priming. Stops and
    // resets deliberately keep it for reuse; disposal is its terminal owner.
    delete_owned_al_source(&mut host_object.al_source, |al_source| unsafe {
        context.DeleteSources(1, &al_source);
        assert!(context.GetError() == 0);
    });

    ns_run_loop::remove_audio_queue(env, host_object.run_loop, in_aq);

    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioQueueNewOutput(_, _, _, _, _, _, _)),
    export_c_func!(AudioQueueGetParameter(_, _, _)),
    export_c_func!(AudioQueueSetParameter(_, _, _)),
    export_c_func!(AudioQueueAllocateBufferWithPacketDescriptions(_, _, _, _)),
    export_c_func!(AudioQueueAllocateBuffer(_, _, _)),
    export_c_func!(AudioQueueEnqueueBuffer(_, _, _, _)),
    export_c_func!(AudioQueueEnqueueBufferWithParameters(
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _
    )),
    export_c_func!(AudioQueueAddPropertyListener(_, _, _, _)),
    export_c_func!(AudioQueueRemovePropertyListener(_, _, _, _)),
    export_c_func!(AudioQueueGetPropertySize(_, _, _)),
    export_c_func!(AudioQueueGetProperty(_, _, _, _)),
    export_c_func!(AudioQueueSetProperty(_, _, _, _)),
    export_c_func!(AudioQueuePrime(_, _, _)),
    export_c_func!(AudioQueueSetOfflineRenderFormat(_, _, _)),
    export_c_func!(AudioQueueOfflineRender(_, _, _, _)),
    export_c_func!(AudioQueueStart(_, _)),
    export_c_func!(AudioQueueGetCurrentTime(_, _, _, _)),
    export_c_func!(AudioQueuePause(_)),
    export_c_func!(AudioQueueStop(_, _)),
    export_c_func!(AudioQueueReset(_)),
    export_c_func!(AudioQueueFlush(_)),
    export_c_func!(AudioQueueFreeBuffer(_, _)),
    export_c_func!(AudioQueueDispose(_, _)),
];

#[cfg(test)]
mod tests {
    use super::{
        audio_queue_buffer_frame_count, delete_owned_al_source, is_supported_audio_format,
        is_supported_audio_queue_format, validate_packet_descriptions, HostPacketDescription,
    };
    use crate::frameworks::core_audio_types::{
        kAudioFormatMPEGLayer3, AudioStreamBasicDescription, AudioStreamPacketDescription,
    };
    use crate::mem::guest_size_of;

    fn mono_game_mp3_format() -> AudioStreamBasicDescription {
        AudioStreamBasicDescription {
            sample_rate: 44_100.0,
            format_id: kAudioFormatMPEGLayer3,
            format_flags: 0,
            bytes_per_packet: 0,
            frames_per_packet: 1152,
            bytes_per_frame: 0,
            channels_per_frame: 2,
            bits_per_channel: 0,
            _reserved: 0,
        }
    }

    fn packet(start_offset: i64, data_byte_size: u32) -> AudioStreamPacketDescription {
        AudioStreamPacketDescription {
            start_offset,
            variable_frames_in_packet: 0,
            data_byte_size,
        }
    }

    #[test]
    fn packet_description_layout_and_bounds_are_validated() {
        assert_eq!(guest_size_of::<AudioStreamPacketDescription>(), 16);

        let descriptions =
            validate_packet_descriptions(835, [packet(0, 417), packet(417, 418)]).unwrap();
        assert_eq!(
            descriptions,
            vec![
                HostPacketDescription {
                    start_offset: 0,
                    variable_frames_in_packet: 0,
                    data_byte_size: 417,
                },
                HostPacketDescription {
                    start_offset: 417,
                    variable_frames_in_packet: 0,
                    data_byte_size: 418,
                },
            ]
        );

        assert!(validate_packet_descriptions(835, []).is_err());
        assert!(validate_packet_descriptions(835, [packet(-1, 417)]).is_err());
        assert!(validate_packet_descriptions(835, [packet(0, 0)]).is_err());
        assert!(validate_packet_descriptions(835, [packet(0, 417), packet(416, 418)]).is_err());
        assert!(validate_packet_descriptions(835, [packet(0, 417), packet(417, 419)]).is_err());
    }

    #[test]
    fn mp3_support_is_limited_to_coherent_audio_queues() {
        let format = mono_game_mp3_format();
        assert!(is_supported_audio_queue_format(&format));
        assert!(!is_supported_audio_format(&format));

        let mut wrong_frames_per_packet = format;
        wrong_frames_per_packet.frames_per_packet = 576;
        assert!(!is_supported_audio_queue_format(&wrong_frames_per_packet));

        let mut wrong_channels = format;
        wrong_channels.channels_per_frame = 3;
        assert!(!is_supported_audio_queue_format(&wrong_channels));

        let mut fixed_packet_size = format;
        fixed_packet_size.bytes_per_packet = 418;
        assert!(!is_supported_audio_queue_format(&fixed_packet_size));
    }

    #[test]
    fn mp3_frame_count_uses_packet_descriptions() {
        let mut descriptions = vec![
            HostPacketDescription {
                start_offset: 0,
                variable_frames_in_packet: 0,
                data_byte_size: 417,
            },
            HostPacketDescription {
                start_offset: 417,
                variable_frames_in_packet: 0,
                data_byte_size: 418,
            },
        ];
        assert_eq!(
            audio_queue_buffer_frame_count(&mono_game_mp3_format(), 835, &descriptions),
            2304
        );

        descriptions[1].variable_frames_in_packet = 576;
        assert_eq!(
            audio_queue_buffer_frame_count(&mono_game_mp3_format(), 835, &descriptions),
            1728
        );
    }

    #[test]
    fn owned_openal_source_is_deleted_exactly_once() {
        let mut source = Some(73);
        let mut deleted_sources = Vec::new();

        delete_owned_al_source(&mut source, |source| deleted_sources.push(source));
        delete_owned_al_source(&mut source, |source| deleted_sources.push(source));

        assert_eq!(source, None);
        assert_eq!(deleted_sources, [73]);
    }
}
