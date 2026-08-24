/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `AudioServices.h` (Audio Services)

use std::collections::HashMap;

use crate::audio::openal as al;
use crate::audio::openal::al_types::*;
use crate::audio::openal::{OpenAL, OpenALManager};

use super::audio_queue::decode_buffer;
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::OSStatus;
use crate::frameworks::core_audio_types::{fourcc, AudioStreamBasicDescription};
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::foundation::ns_url::to_rust_path;
use crate::mem::{ConstPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::{audio, Environment};

/// Usually a FourCC.
type AudioServicesPropertyID = u32;
type SystemSoundID = u32;

const kAudioServicesUnsupportedPropertyError: OSStatus = fourcc(b"pty?") as _;
const kSystemSoundID_Vibrate: SystemSoundID = 0x00000FFF;
const kAudioServicesSystemSoundUnspecifiedError: OSStatus = -1500;

const INITIAL_SYSTEM_SOUND_ID: SystemSoundID = 0x1001;

/// `void (*)(SystemSoundID inSystemSoundID, void *inClientData)`.
type AudioServicesSystemSoundCompletionProc = GuestFunction;

struct SystemSoundCompletion {
    proc: AudioServicesSystemSoundCompletionProc,
    client_data: MutVoidPtr,
    /// Whether the source was playing the last time the run loop looked. The
    /// completion fires on the playing-to-stopped edge, so a sound that was
    /// never started does not immediately call back.
    was_playing: bool,
}

struct SystemSoundData {
    al_source: ALuint,
    al_buffer: ALuint,
    completion: Option<SystemSoundCompletion>,
}

pub struct State {
    sounds: HashMap<SystemSoundID, SystemSoundData>,
    data_top: SystemSoundID,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sounds: Default::default(),
            data_top: INITIAL_SYSTEM_SOUND_ID,
        }
    }
}

impl State {
    fn get_with_context<'s, 'm: 's>(
        framework_state: &'s mut crate::frameworks::State,
        manager: &'m mut OpenALManager,
    ) -> (&'s mut Self, OpenAL<'s>) {
        let toolbox = &mut framework_state.audio_toolbox;
        (
            &mut toolbox.audio_services,
            toolbox.al_context.make_al_context_current(manager),
        )
    }
}

fn AudioServicesCreateSystemSoundID(
    env: &mut Environment,
    file: CFURLRef,
    out_system_sound_id: MutPtr<SystemSoundID>,
) -> OSStatus {
    let path = to_rust_path(env, file);
    let Ok(mut audio_file) = audio::AudioFile::open_for_reading(&path, &env.fs) else {
        log!(
            "Warning: Failed to open audio file {:?} for AudioServicesCreateSystemSoundID()",
            path
        );
        return kAudioServicesSystemSoundUnspecifiedError;
    };

    let mut data = vec![0; audio_file.byte_count().try_into().unwrap()];
    let format =
        AudioStreamBasicDescription::from_audio_description(audio_file.audio_description());
    let size = audio_file.read_bytes(0, data.as_mut_slice()).unwrap();
    let tmp = env.mem.alloc(size as GuestUSize);
    env.mem
        .bytes_at_mut(tmp.cast(), size as GuestUSize)
        .copy_from_slice(data.as_slice());
    let (al_format, al_frequency, data) =
        decode_buffer(&env.mem, &format, tmp.cast(), size as GuestUSize);
    env.mem.free(tmp.cast());

    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    // TODO: This should only support linear pcm and ima4, but also supports
    // mp3 here since AudioFile supports it. We also aren't checking for length.
    let mut al_source = 0;
    unsafe {
        context.GenSources(1, &mut al_source);
        assert!(context.GetError() == 0);
    }

    let mut al_buffer = 0;
    unsafe {
        context.GenBuffers(1, &mut al_buffer);
        context.BufferData(
            al_buffer,
            al_format,
            data.as_ptr() as *const ALvoid,
            data.len().try_into().unwrap(),
            al_frequency,
        );
        context.Sourcei(al_source, al::AL_BUFFER, al_buffer.try_into().unwrap());
        assert!(context.GetError() == 0);
    }
    let sys_data = SystemSoundData {
        al_source,
        al_buffer,
        completion: None,
    };
    state.sounds.insert(state.data_top, sys_data);
    env.mem.write(out_system_sound_id, state.data_top);
    state.data_top = state.data_top.checked_add(1).unwrap();
    0
}

fn AudioServicesGetProperty(
    _env: &mut Environment,
    in_property_id: AudioServicesPropertyID,
    _in_specifier_size: u32,
    _in_specifier: crate::mem::ConstVoidPtr,
    _io_property_data_size: MutPtr<u32>,
    _out_property_data: MutVoidPtr,
) -> OSStatus {
    // Crash Bandicoot Nitro Kart 3D tries to use this property ID, which does
    // not seem to be documented anywhere? Assuming this is a bug.
    if in_property_id == 0xfff {
        kAudioServicesUnsupportedPropertyError
    } else {
        unimplemented!();
    }
}

fn AudioServicesPlaySystemSound(env: &mut Environment, sys_sound_id: SystemSoundID) {
    if sys_sound_id == kSystemSoundID_Vibrate {
        log!("TODO: vibration (AudioServicesPlaySystemSound)");
    } else {
        let (state, context) =
            State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

        if let Some(SystemSoundData {
            al_source,
            al_buffer: _,
            completion: _,
        }) = state.sounds.get(&sys_sound_id)
        {
            unsafe {
                let al_source = *al_source;
                context.SourcePlay(al_source);
                assert!(context.GetError() == 0);
                let mut al_state: i32 = 0;
                context.GetSourcei(al_source, al::AL_SOURCE_STATE, &mut al_state as *mut i32);
                assert!(context.GetError() == 0);
                assert!(
                    al_state == al::AL_PLAYING,
                    "Expected AL_PLAYING after SourcePlay, got {:#x}",
                    al_state
                );
            }
        } else {
            // An ID tapHLE never handed out, or one already disposed. The API
            // returns void and has no way to report this, and the real
            // framework simply does not play anything, so neither does this.
            // Aborting here turned a silent sound into a dead app.
            log!(
                "Warning: AudioServicesPlaySystemSound({:#x}) names a sound that was never created, ignoring",
                sys_sound_id
            );
        }
    }
    // TODO: implement other system sounds
}

fn AudioServicesDisposeSystemSoundID(
    env: &mut Environment,
    sys_sound_id: SystemSoundID,
) -> OSStatus {
    let (state, context) =
        State::get_with_context(&mut env.framework_state, &mut env.openal_manager);

    if let Some(SystemSoundData {
        al_source,
        al_buffer,
        completion: _,
    }) = state.sounds.remove(&sys_sound_id)
    {
        unsafe {
            context.SourceStop(al_source);
            context.DeleteSources(1, &al_source as *const ALuint);
            context.DeleteBuffers(1, &al_buffer as *const ALuint);
            assert!(context.GetError() == 0);
        }
        0
    } else {
        // This is also true of kSystemSoundID_Vibrate.
        log!("Tried to dispose of invalid system sound {}!", sys_sound_id);
        kAudioServicesSystemSoundUnspecifiedError
    }
}

/// Registers a callback to run once the sound finishes playing.
///
/// The run loop and mode arguments say where the callback should be scheduled.
/// tapHLE only runs one run loop, and this is driven from it (see
/// [handle_system_sound_completions]), so they are accepted and ignored.
fn AudioServicesAddSystemSoundCompletion(
    env: &mut Environment,
    sys_sound_id: SystemSoundID,
    _in_run_loop: MutVoidPtr,        // CFRunLoopRef
    _in_run_loop_mode: ConstPtr<()>, // CFStringRef
    in_completion_routine: AudioServicesSystemSoundCompletionProc,
    in_client_data: MutVoidPtr,
) -> OSStatus {
    let state = &mut env.framework_state.audio_toolbox.audio_services;
    let Some(sound) = state.sounds.get_mut(&sys_sound_id) else {
        if sys_sound_id == kSystemSoundID_Vibrate {
            // A completion on the vibration "sound" is legitimate, but there is
            // nothing here to finish, because vibration itself is a TODO. The
            // routine simply never runs.
            log!("TODO: completion for vibration (AudioServicesAddSystemSoundCompletion)");
        } else {
            log!(
                "Tried to add a completion to invalid system sound {}!",
                sys_sound_id
            );
        }
        return kAudioServicesSystemSoundUnspecifiedError;
    };
    sound.completion = Some(SystemSoundCompletion {
        proc: in_completion_routine,
        client_data: in_client_data,
        was_playing: false,
    });
    0
}

fn AudioServicesRemoveSystemSoundCompletion(env: &mut Environment, sys_sound_id: SystemSoundID) {
    let state = &mut env.framework_state.audio_toolbox.audio_services;
    if let Some(sound) = state.sounds.get_mut(&sys_sound_id) {
        sound.completion = None;
    }
}

/// Call the completion routine of every system sound that has finished playing
/// since the last check. Called from the run loop.
pub fn handle_system_sound_completions(env: &mut Environment) {
    // Collect first: a completion routine is guest code, which may play, add,
    // remove or dispose sounds, so the map must not be borrowed across the
    // call.
    let mut finished = Vec::new();
    {
        let (state, context) =
            State::get_with_context(&mut env.framework_state, &mut env.openal_manager);
        for (&sys_sound_id, sound) in state.sounds.iter_mut() {
            let Some(completion) = &mut sound.completion else {
                continue;
            };
            let mut al_state: ALint = 0;
            unsafe {
                context.GetSourcei(sound.al_source, al::AL_SOURCE_STATE, &mut al_state);
                assert!(context.GetError() == 0);
            }
            let playing = al_state == al::AL_PLAYING;
            if completion.was_playing && !playing {
                finished.push((sys_sound_id, completion.proc, completion.client_data));
            }
            completion.was_playing = playing;
        }
    }
    for (sys_sound_id, proc, client_data) in finished {
        log_dbg!(
            "Calling system sound {} completion {:?}",
            sys_sound_id,
            proc
        );
        let _: () = proc.call_from_host(env, (sys_sound_id, client_data));
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioServicesAddSystemSoundCompletion(_, _, _, _, _)),
    export_c_func!(AudioServicesRemoveSystemSoundCompletion(_)),
    export_c_func!(AudioServicesCreateSystemSoundID(_, _)),
    export_c_func!(AudioServicesGetProperty(_, _, _, _, _)),
    export_c_func!(AudioServicesPlaySystemSound(_)),
    export_c_func!(AudioServicesDisposeSystemSoundID(_)),
];
