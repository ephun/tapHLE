/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ExtendedAudioFile.h` (Extended Audio File Services)
//!
//! Currently implemented as a dummy wrapper around Audio File Services.

// TODO: Audio format conversion

use super::audio_file::{
    kAudioFileBadPropertySizeError, kAudioFilePropertyDataFormat, kAudioFileReadPermission,
    property_size, AudioFileClose, AudioFileGetProperty, AudioFileID, AudioFileOpenURL,
    AudioFileReadBytes,
};
use super::audio_queue::is_supported_audio_format;
use super::audio_unit::AudioBufferList;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::{eofErr, paramErr, OSStatus};
use crate::frameworks::core_audio_types::{
    debug_fourcc, fourcc, kAudioFormatLinearPCM, AudioStreamBasicDescription,
};
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::foundation::ns_url::to_rust_path;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub extended_audio_files: HashMap<ExtAudioFileRef, ExtAudioFileHostObject>,
}
impl State {
    pub fn get(framework_state: &mut crate::frameworks::State) -> &mut Self {
        &mut framework_state.audio_toolbox.extended_audio_file
    }
}

pub struct ExtAudioFileHostObject {
    guest_audio_file: AudioFileID,
    client_data_format: Option<AudioStreamBasicDescription>,
    current_bytes_read: i64,
}

#[repr(C, packed)]
pub struct OpaqueExtAudioFile {
    _filler: u8,
}
unsafe impl SafeRead for OpaqueExtAudioFile {}

type ExtAudioFileRef = MutPtr<OpaqueExtAudioFile>;

/// Usually a FourCC.
type ExtAudioFilePropertyID = u32;
const kExtAudioFileProperty_FileDataFormat: ExtAudioFilePropertyID = fourcc(b"ffmt");
const kExtAudioFileProperty_FileLengthFrames: ExtAudioFilePropertyID = fourcc(b"#frm");
const kExtAudioFileProperty_ClientDataFormat: ExtAudioFilePropertyID = fourcc(b"cfmt");

fn ExtAudioFileOpenURL(
    env: &mut Environment,
    in_url: CFURLRef,
    out_ext_audio_file: MutPtr<ExtAudioFileRef>,
) -> OSStatus {
    return_if_null!(in_url);

    let path = to_rust_path(env, in_url);
    log_dbg!(
        "ExtAudioFileOpenURL({:?} '{:?}', {:?})",
        in_url,
        path,
        out_ext_audio_file
    );

    let audio_file_ptr: MutPtr<AudioFileID> = env.mem.alloc(guest_size_of::<AudioFileID>()).cast();
    let res = AudioFileOpenURL(env, in_url, kAudioFileReadPermission, 0, audio_file_ptr);
    let guest_audio_file = env.mem.read(audio_file_ptr);
    env.mem.free(audio_file_ptr.cast());
    if res != 0 {
        log!(
            "ExtAudioFileOpenURL({:?} '{:?}', {:?}) failed, error: {:?}",
            in_url,
            path,
            out_ext_audio_file,
            res
        );
        return res;
    }

    let host_object = ExtAudioFileHostObject {
        guest_audio_file,
        client_data_format: None,
        current_bytes_read: 0,
    };

    let guest_extended_audio_file = env.mem.alloc_and_write(OpaqueExtAudioFile { _filler: 0 });
    State::get(&mut env.framework_state)
        .extended_audio_files
        .insert(guest_extended_audio_file, host_object);

    env.mem.write(out_ext_audio_file, guest_extended_audio_file);

    0 // success
}

fn ExtAudioFileGetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    io_property_data_size: MutPtr<u32>,
    out_property_data: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let required_size = match in_property_id {
        kExtAudioFileProperty_FileDataFormat => property_size(kAudioFilePropertyDataFormat),
        kExtAudioFileProperty_FileLengthFrames => guest_size_of::<i64>(),
        _ => unimplemented!(
            "Unimplemented property ID: {}",
            debug_fourcc(in_property_id)
        ),
    };
    if env.mem.read(io_property_data_size) != required_size {
        log!("Warning: ExtAudioFileGetProperty() failed");
        return kAudioFileBadPropertySizeError;
    }

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    match in_property_id {
        kExtAudioFileProperty_FileDataFormat => AudioFileGetProperty(
            env,
            host_object.guest_audio_file,
            kAudioFilePropertyDataFormat,
            io_property_data_size,
            out_property_data,
        ),
        kExtAudioFileProperty_FileLengthFrames => {
            let audio_file = &env
                .framework_state
                .audio_toolbox
                .audio_file
                .audio_files
                .get(&host_object.guest_audio_file)
                .unwrap()
                .audio_file;
            let frames = audio_file
                .packet_count()
                .checked_mul(u64::from(audio_file.audio_description().frames_per_packet))
                .unwrap();
            env.mem
                .write(out_property_data.cast(), i64::try_from(frames).unwrap());
            0
        }
        _ => unreachable!(),
    }
}

fn ExtAudioFileSetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    in_property_data_size: u32,
    in_property_data: ConstVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    assert_eq!(in_property_id, kExtAudioFileProperty_ClientDataFormat);
    assert_eq!(
        in_property_data_size,
        guest_size_of::<AudioStreamBasicDescription>()
    );

    let audio_desc_ptr: ConstPtr<AudioStreamBasicDescription> = in_property_data.cast();
    let client_audio_desc = env.mem.read(audio_desc_ptr);
    log_dbg!("ExtAudioFileSetProperty {:?}", client_audio_desc);
    let format_id = client_audio_desc.format_id;
    assert_eq!(format_id, kAudioFormatLinearPCM);
    assert!(is_supported_audio_format(&client_audio_desc));

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();
    assert!(host_object.client_data_format.is_none());
    host_object.client_data_format = Some(client_audio_desc);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();
    let other_host_object = env
        .framework_state
        .audio_toolbox
        .audio_file
        .audio_files
        .get(&host_object.guest_audio_file)
        .unwrap();
    let audio_desc = AudioStreamBasicDescription::from_audio_description(
        other_host_object.audio_file.audio_description(),
    );
    // TODO: support audio format conversions
    assert_eq!(audio_desc, client_audio_desc);

    0 // success
}

fn ExtAudioFileRead(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    io_number_frames: MutPtr<u32>,
    io_data: MutPtr<AudioBufferList<1>>,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let mut audio_buffer_list = env.mem.read(io_data);
    let num_buffers = audio_buffer_list.number_buffers;
    assert_eq!(num_buffers, 1);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    audio_buffer_list.buffers[0].number_channels =
        host_object.client_data_format.unwrap().channels_per_frame;

    let number_frames = env.mem.read(io_number_frames);
    let bytes_per_frame = host_object.client_data_format.unwrap().bytes_per_frame;
    let number_of_bytes = number_frames.checked_mul(bytes_per_frame).unwrap();
    let number_of_bytes_ptr = env.mem.alloc_and_write(number_of_bytes);
    let res = AudioFileReadBytes(
        env,
        host_object.guest_audio_file,
        false,
        host_object.current_bytes_read,
        number_of_bytes_ptr,
        audio_buffer_list.buffers[0].data,
    );
    let number_of_bytes_read = env.mem.read(number_of_bytes_ptr);
    env.mem.free(number_of_bytes_ptr.cast());
    if res != 0 {
        if res == eofErr {
            env.mem.write(io_number_frames, 0);
            return 0;
        }
        log!(
            "ExtAudioFileRead({:?}, {:?}, {:?}) failed, error: {:?}",
            in_ext_audio_file,
            io_number_frames,
            io_data,
            res
        );
        return res;
    }
    env.mem.write(
        io_number_frames,
        number_of_bytes_read.checked_div(bytes_per_frame).unwrap(),
    );
    audio_buffer_list.buffers[0].data_byte_size = number_of_bytes_read;

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();
    host_object.current_bytes_read += number_of_bytes_read as i64;

    env.mem.write(io_data, audio_buffer_list);

    0 // success
}

/// Move the read cursor to a frame offset.
///
/// The offset is in frames of the *client* format — the format the app asked to
/// be given, not the one on disk — which is the same unit `ExtAudioFileRead`
/// counts in, so this is the byte cursor that already exists expressed
/// differently. A file whose client format has not been set yet has no frame
/// size to convert with, and Apple's implementation refuses that too.
fn ExtAudioFileSeek(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_frame_offset: i64,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();

    let Some(client_data_format) = host_object.client_data_format else {
        log!(
            "ExtAudioFileSeek({:?}) before the client data format was set",
            in_ext_audio_file
        );
        return paramErr;
    };

    // A negative offset has no meaning and would put the cursor behind the
    // start of the file, where AudioFileReadBytes would read whatever happened
    // to precede it.
    let Ok(frame_offset) = u32::try_from(in_frame_offset) else {
        log!(
            "ExtAudioFileSeek({:?}, {}) out of range",
            in_ext_audio_file,
            in_frame_offset
        );
        return paramErr;
    };

    // Seeking past the end is not an error: the next read simply reports zero
    // frames, which is how ExtAudioFileRead already handles hitting the end.
    host_object.current_bytes_read =
        i64::from(frame_offset) * i64::from(client_data_format.bytes_per_frame);

    log_dbg!(
        "ExtAudioFileSeek({:?}, {}) => byte {}",
        in_ext_audio_file,
        in_frame_offset,
        host_object.current_bytes_read
    );

    0 // success
}

fn ExtAudioFileDispose(env: &mut Environment, in_ext_audio_file: ExtAudioFileRef) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
        .unwrap();

    let res = AudioFileClose(env, host_object.guest_audio_file);
    assert_eq!(res, 0); // success

    let _host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .remove(&in_ext_audio_file)
        .unwrap();
    env.mem.free(in_ext_audio_file.cast());

    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(ExtAudioFileOpenURL(_, _)),
    export_c_func!(ExtAudioFileGetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileSetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileRead(_, _, _)),
    export_c_func!(ExtAudioFileSeek(_, _)),
    export_c_func!(ExtAudioFileDispose(_)),
];
