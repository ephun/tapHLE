/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `AudioSession.h` (Audio Session) // TODO: is this the real name?

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::OSStatus;
use crate::frameworks::core_audio_types::{debug_fourcc, fourcc};
use crate::frameworks::core_foundation::cf_run_loop::{CFRunLoopMode, CFRunLoopRef};
use crate::frameworks::foundation::ns_string;
use crate::mem::{guest_size_of, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::Environment;

type AudioSessionInterruptionListener = GuestFunction;
type AudioSessionPropertyListener = GuestFunction;

const kAudioSessionBadPropertySizeError: OSStatus = fourcc(b"!siz") as _;

/// Usually a FourCC.
type AudioSessionPropertyID = u32;
const kAudioSessionProperty_OtherAudioIsPlaying: AudioSessionPropertyID = fourcc(b"othr");
const kAudioSessionProperty_AudioCategory: AudioSessionPropertyID = fourcc(b"acat");
const kAudioSessionProperty_CurrentHardwareSampleRate: AudioSessionPropertyID = fourcc(b"chsr");
const kAudioSessionProperty_CurrentHardwareOutputNumberChannels: AudioSessionPropertyID =
    fourcc(b"choc");
const kAudioSessionProperty_CurrentHardwareOutputVolume: AudioSessionPropertyID = fourcc(b"chov");
const kAudioSessionProperty_PreferredHardwareIOBufferDuration: AudioSessionPropertyID =
    fourcc(b"iobd");
const kAudioSessionProperty_PreferredHardwareSampleRate: AudioSessionPropertyID = fourcc(b"hwsr");

const kAudioSessionCategory_SoloAmbientSound: u32 = fourcc(b"solo");
const kAudioSessionProperty_CurrentHardwareIOBufferDuration: u32 = fourcc(b"chbd");
/// Set by apps that want their audio to mix with whatever else is playing.
/// There is nothing else playing here, so the answer is the same either way —
/// but the call has to be accepted, because refusing it stopped seven apps in a
/// 1501-app survey.
const kAudioSessionProperty_OverrideCategoryMixWithOthers: AudioSessionPropertyID = fourcc(b"cmix");
/// The current audio route, as a CFStringRef. Read by apps deciding whether to
/// duck or pause when headphones are unplugged.
const kAudioSessionProperty_AudioRoute: AudioSessionPropertyID = fourcc(b"rout");

pub struct State {
    audio_session_category: u32,
    pub current_hardware_sample_rate: f64,
    pub current_hardware_output_number_channels: u32,
    current_hardware_output_volume: f32,
    current_hardware_io_buffer_duration: f32,
}
impl Default for State {
    fn default() -> Self {
        // TODO: Check values from a real device
        State {
            // This is the default value.
            audio_session_category: kAudioSessionCategory_SoloAmbientSound,
            // Values taken from an iOS 2 simulator
            current_hardware_sample_rate: 44100.0,
            current_hardware_output_number_channels: 2,
            current_hardware_output_volume: 1.0,
            // Value was checked on both iOS Simulator and iPhone 3GS
            current_hardware_io_buffer_duration: 0.023220,
        }
    }
}

fn AudioSessionInitialize(
    _env: &mut Environment,
    in_run_loop: CFRunLoopRef,
    in_run_loop_mode: CFRunLoopMode,
    in_interruption_listener: AudioSessionInterruptionListener,
    in_client_data: MutVoidPtr,
) -> OSStatus {
    let result = 0; // success
    log!(
        "TODO: AudioSessionInitialize({:?}, {:?}, {:?}, {:?}) -> {:?}",
        in_run_loop,
        in_run_loop_mode,
        in_interruption_listener,
        in_client_data,
        result
    );
    result
}

fn AudioSessionGetPropertySize(
    env: &mut Environment,
    in_ID: AudioSessionPropertyID,
    out_data_size: MutPtr<u32>,
) -> OSStatus {
    let size = get_audio_session_property_size(in_ID);
    env.mem.write(out_data_size, size);
    0 // Success
}

fn AudioSessionGetProperty(
    env: &mut Environment,
    in_ID: AudioSessionPropertyID,
    io_data_size: MutPtr<u32>,
    out_data: MutVoidPtr,
) -> OSStatus {
    let required_size = get_audio_session_property_size(in_ID);
    let io_data_size_value = env.mem.read(io_data_size);
    if io_data_size_value != required_size {
        log!("Warning: AudioSessionGetProperty() failed");
        return kAudioSessionBadPropertySizeError;
    }

    let state = &env.framework_state.audio_toolbox.audio_session;
    match in_ID {
        kAudioSessionProperty_OtherAudioIsPlaying => {
            let value: u32 = 0;
            env.mem.write(out_data.cast(), value);
        }
        kAudioSessionProperty_AudioCategory => {
            let value: u32 = state.audio_session_category;
            env.mem.write(out_data.cast(), value);
        }
        kAudioSessionProperty_CurrentHardwareSampleRate => {
            let value: f64 = state.current_hardware_sample_rate;
            env.mem.write(out_data.cast(), value);
        }
        kAudioSessionProperty_CurrentHardwareOutputNumberChannels => {
            let value: u32 = state.current_hardware_output_number_channels;
            env.mem.write(out_data.cast(), value);
        }
        kAudioSessionProperty_CurrentHardwareOutputVolume => {
            let value: f32 = state.current_hardware_output_volume;
            env.mem.write(out_data.cast(), value);
        }
        kAudioSessionProperty_AudioRoute => {
            // There is one output and nothing can be plugged into it, so the
            // route is always the speaker. Apps read this to decide whether to
            // pause when headphones are removed, which cannot happen here.
            let route = ns_string::get_static_str(env, "Speaker");
            env.mem.write(out_data.cast(), route);
        }
        kAudioSessionProperty_CurrentHardwareIOBufferDuration => {
            let value: f32 = state.current_hardware_io_buffer_duration;
            env.mem.write(out_data.cast(), value);
        }
        _ => unreachable!(),
    }

    let result = 0; // success
    log_dbg!(
        "AudioSessionGetProperty({:?}, {:?} ({:?}), {:?} ({:?})) -> {:?})",
        in_ID,
        io_data_size,
        io_data_size_value,
        out_data,
        env.mem.bytes_at(out_data.cast(), io_data_size_value),
        result
    );
    result
}

fn AudioSessionSetProperty(
    env: &mut Environment,
    in_ID: AudioSessionPropertyID,
    in_data_size: u32,
    in_data: ConstVoidPtr,
) -> OSStatus {
    let required_size: GuestUSize = match in_ID {
        kAudioSessionProperty_AudioCategory => guest_size_of::<u32>(),
        kAudioSessionProperty_PreferredHardwareIOBufferDuration => guest_size_of::<f32>(),
        kAudioSessionProperty_PreferredHardwareSampleRate => guest_size_of::<f64>(),
        kAudioSessionProperty_OverrideCategoryMixWithOthers => guest_size_of::<u32>(),
        // An unknown property is not worth dying over. Audio Session settings
        // are preferences about routing and mixing, and tapHLE honours almost
        // none of them already — refusing to accept one it does not recognise
        // stops the app without improving the audio.
        _ => {
            log!(
                "TODO: ignoring AudioSessionSetProperty for unimplemented property ID: {}",
                debug_fourcc(in_ID)
            );
            return 0;
        }
    };
    if in_data_size != required_size {
        log!("Warning: AudioSessionSetProperty() failed");
        return kAudioSessionBadPropertySizeError;
    }
    if in_ID == kAudioSessionProperty_PreferredHardwareSampleRate {
        env.framework_state
            .audio_toolbox
            .audio_session
            .current_hardware_sample_rate = env.mem.read(in_data.cast::<f64>());
        log!(
            "AudioSessionSetProperty current_hardware_sample_rate {}",
            env.framework_state
                .audio_toolbox
                .audio_session
                .current_hardware_sample_rate
        );
    }

    let result = 0; // success
    log!(
        "TODO: AudioSessionSetProperty({:?}, {:?}, {:?} ({:?})) -> {:?}",
        in_ID,
        in_data_size,
        in_data,
        env.mem.bytes_at(in_data.cast(), in_data_size),
        result
    );
    result
}

fn AudioSessionSetActive(_env: &mut Environment, active: bool) -> OSStatus {
    let result = 0; // success
    log!("TODO: AudioSessionSetActive({:?}) -> {:?}", active, result);
    result
}

fn AudioSessionAddPropertyListener(
    _env: &mut Environment,
    inID: AudioSessionPropertyID,
    inProc: AudioSessionPropertyListener,
    inClientData: MutVoidPtr,
) -> OSStatus {
    let result = 0; // success
    log!(
        "TODO: AudioSessionAddPropertyListener({:?}, {:?}, {:?}) -> {}",
        inID,
        inProc,
        inClientData,
        result
    );
    result
}
fn AudioSessionRemovePropertyListenerWithUserData(
    _env: &mut Environment,
    in_property_id: AudioSessionPropertyID,
    in_listener: AudioSessionPropertyListener,
    in_client_data: MutVoidPtr,
) -> OSStatus {
    let result = 0; // success
    log!(
        "TODO: AudioSessionRemovePropertyListenerWithUserData({:?}, {:?}, {:?}) -> {}",
        in_property_id,
        in_listener,
        in_client_data,
        result
    );
    result
}

/// Helper function to get AudioSession Property size by id
fn get_audio_session_property_size(in_ID: AudioSessionPropertyID) -> GuestUSize {
    match in_ID {
        kAudioSessionProperty_OtherAudioIsPlaying => guest_size_of::<u32>(),
        kAudioSessionProperty_AudioCategory => guest_size_of::<u32>(),
        kAudioSessionProperty_CurrentHardwareSampleRate => guest_size_of::<f64>(),
        kAudioSessionProperty_CurrentHardwareOutputNumberChannels => guest_size_of::<u32>(),
        kAudioSessionProperty_CurrentHardwareOutputVolume => guest_size_of::<f32>(),
        kAudioSessionProperty_CurrentHardwareIOBufferDuration => guest_size_of::<f32>(),
        // A CFStringRef.
        kAudioSessionProperty_AudioRoute => guest_size_of::<MutVoidPtr>(),
        // Zero means "no such property", which makes the caller's size check
        // fail and produces the documented kAudioSessionBadPropertySizeError.
        // That is an answer the app is written to handle; aborting is not.
        _ => 0,
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioSessionInitialize(_, _, _, _)),
    export_c_func!(AudioSessionGetProperty(_, _, _)),
    export_c_func!(AudioSessionGetPropertySize(_, _)),
    export_c_func!(AudioSessionSetProperty(_, _, _)),
    export_c_func!(AudioSessionSetActive(_)),
    export_c_func!(AudioSessionAddPropertyListener(_, _, _)),
    export_c_func!(AudioSessionRemovePropertyListenerWithUserData(_, _, _)),
];
