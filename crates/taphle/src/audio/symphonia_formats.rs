/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Quick-and-dirty decoding of miscellaneous formats (MP3, AAC) to linear PCM.
//!
//! This should be the only module in tapHLE that makes use of [symphonia].
//! For AAC, Only the LC profile and MPEG-4 container format are supported (see
//! feature list in Cargo.toml).

use std::io::Cursor;
use symphonia::core::audio::AudioSpec;
use symphonia::core::codecs::audio::{
    well_known::{
        CODEC_ID_AAC, CODEC_ID_ADPCM_IMA_QT, CODEC_ID_ADPCM_IMA_WAV, CODEC_ID_ALAC, CODEC_ID_MP3,
        CODEC_ID_PCM_S16BE, CODEC_ID_PCM_S16LE,
    },
    AudioCodecParameters, AudioDecoder, AudioDecoderOptions,
};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::packet::PacketRef;
use symphonia::core::units::{Duration, Timestamp};

/// PCM data decoded from an miscellaneous format file.
pub struct SymphoniaDecodedToPcm {
    /// 16-bit little-endian PCM samples, grouped in frames (one sample per
    /// channel in each frame).
    pub bytes: Vec<u8>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u32,
}

/// Stateful decoder for a packetized MP3 stream.
///
/// MP3 packets must be decoded in stream order because later packets may use
/// the Layer III bit reservoir populated by earlier packets. One instance
/// should therefore be retained for the lifetime of its audio queue.
pub struct StreamingMp3Decoder {
    decoder: Box<dyn AudioDecoder>,
    expected_sample_rate: u32,
    expected_channels: u32,
    next_pts: i64,
}

impl StreamingMp3Decoder {
    pub fn new(expected_sample_rate: u32, expected_channels: u32) -> Result<Self, String> {
        if expected_sample_rate == 0 {
            return Err("MP3 sample rate must be non-zero".to_owned());
        }
        if !matches!(expected_channels, 1 | 2) {
            return Err(format!(
                "MP3 channel count must be one or two, got {expected_channels}"
            ));
        }

        let mut codec_params = AudioCodecParameters::new();
        codec_params.for_codec(CODEC_ID_MP3);
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
            .map_err(|error| format!("Could not create MP3 decoder: {error}"))?;

        Ok(Self {
            decoder,
            expected_sample_rate,
            expected_channels,
            next_pts: 0,
        })
    }

    /// Decode one complete MP3 packet to interleaved native-endian i16 PCM.
    pub fn decode_packet(&mut self, encoded: &[u8], frame_count: u32) -> Result<Vec<u8>, String> {
        if encoded.is_empty() {
            return Err("Cannot decode an empty MP3 packet".to_owned());
        }
        if frame_count == 0 {
            return Err("MP3 packet frame count must be non-zero".to_owned());
        }

        let pts = self.next_pts;
        self.next_pts = self
            .next_pts
            .checked_add(i64::from(frame_count))
            .ok_or_else(|| "MP3 packet timestamp overflow".to_owned())?;

        let packet = PacketRef::new(
            0,
            Timestamp::new(pts),
            Duration::from(u64::from(frame_count)),
            encoded,
        );
        let decoded = self
            .decoder
            .decode_ref(&packet)
            .map_err(|error| format!("Could not decode MP3 packet: {error}"))?;

        let actual_sample_rate = decoded.spec().rate();
        let actual_channels: u32 = decoded
            .spec()
            .channels()
            .count()
            .try_into()
            .map_err(|_| "Decoded MP3 channel count does not fit in u32".to_owned())?;
        if actual_sample_rate != self.expected_sample_rate
            || actual_channels != self.expected_channels
        {
            return Err(format!(
                "Decoded MP3 format is {actual_sample_rate} Hz/{actual_channels} channels, expected {} Hz/{} channels",
                self.expected_sample_rate, self.expected_channels
            ));
        }

        let mut pcm = Vec::with_capacity(decoded.byte_len_as::<i16>());
        decoded.copy_bytes_to_vec_interleaved_as::<i16>(&mut pcm);
        Ok(pcm)
    }

    /// Reset all cross-packet codec state after a stream discontinuity.
    pub fn reset(&mut self) {
        self.decoder.reset();
        self.next_pts = 0;
    }
}

pub fn decode_symphonia_to_pcm(file: Cursor<Vec<u8>>) -> Result<SymphoniaDecodedToPcm, ()> {
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // If this failed, the container format is not supported.
    let mut probed = symphonia::default::get_probe()
        .probe(
            &Default::default(),
            mss,
            Default::default(),
            Default::default(),
        )
        .map_err(|_| ())?;

    let track = probed
        .tracks()
        .iter()
        .find(|t| {
            if let Some(codec_params) = &t.codec_params {
                if let Some(audio_codec_params) = codec_params.audio() {
                    audio_codec_params.codec == CODEC_ID_AAC
                        || audio_codec_params.codec == CODEC_ID_ADPCM_IMA_WAV
                        || audio_codec_params.codec == CODEC_ID_ADPCM_IMA_QT
                        || audio_codec_params.codec == CODEC_ID_ALAC
                        || audio_codec_params.codec == CODEC_ID_MP3
                        || audio_codec_params.codec == CODEC_ID_PCM_S16BE
                        || audio_codec_params.codec == CODEC_ID_PCM_S16LE
                } else {
                    false
                }
            } else {
                false
            }
        })
        .ok_or(())?;
    let track_id = track.id;

    // Not sure why this would fail, maybe an unusual AAC track.
    let audio_codec_params = track.codec_params.as_ref().unwrap().audio().unwrap();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_codec_params, &Default::default())
        .map_err(|_| ())?;

    let mut out_pcm = Vec::<u8>::new();
    let mut audio_spec: Option<AudioSpec> = None;
    {
        let mut tmp_raw_s16_buf: Option<Vec<u8>> = None;
        loop {
            let packet = match probed.next_packet() {
                Ok(packet) => match packet {
                    Some(packet) => packet,
                    // "If Ok(None) is returned, the media has ended and
                    // no more packets will be produced until the reader
                    // is seeked to a new position."
                    None => break,
                },
                // Assume I/O errors can only mean end-of-file, because the
                // entire file is in-memory.
                Err(symphonia::core::errors::Error::IoError(_)) => break,
                Err(_) => return Err(()),
            };

            if packet.track_id != track_id {
                continue;
            }
            let Ok(decoded_packet) = decoder.decode(&packet) else {
                break;
            };

            // For some reason, the "audio spec" (number of channels etc)
            // is reported per-packet? This is weird because it must be the same
            // for all of them.
            let audio_spec = audio_spec.get_or_insert_with(|| decoded_packet.spec().clone());
            assert_eq!(audio_spec, decoded_packet.spec());

            // Note that this assumes every packet's buffer's capacity is the
            // same, which is a dubious assumption, but Symphonia's own example
            // code does it, so maybe it's fine?
            let tmp_raw_s16_buf = tmp_raw_s16_buf
                .get_or_insert_with(|| Vec::with_capacity(decoded_packet.capacity()));
            tmp_raw_s16_buf.clear();
            decoded_packet.copy_bytes_to_vec_interleaved_as::<i16>(tmp_raw_s16_buf);

            out_pcm.extend_from_slice(tmp_raw_s16_buf);
        }
    }
    let audio_spec = audio_spec.ok_or(())?;
    Ok(SymphoniaDecodedToPcm {
        bytes: out_pcm,
        sample_rate: audio_spec.rate(),
        channels: audio_spec.channels().count().try_into().unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::StreamingMp3Decoder;

    fn synthetic_silent_mp3_frame(padded: bool) -> Vec<u8> {
        // MPEG-1 Layer III, 128 kbit/s, 44.1 kHz, joint stereo. At this
        // bitrate the frame is 417 bytes, or 418 bytes with padding.
        let mut frame = vec![0; if padded { 418 } else { 417 }];
        frame[..4].copy_from_slice(if padded {
            &[0xff, 0xfb, 0x92, 0x64]
        } else {
            &[0xff, 0xfb, 0x90, 0x64]
        });
        frame
    }

    #[test]
    fn streaming_mp3_decoder_decodes_variable_size_frames() {
        let mut decoder = StreamingMp3Decoder::new(44_100, 2).unwrap();

        let unpadded = decoder
            .decode_packet(&synthetic_silent_mp3_frame(false), 1152)
            .unwrap();
        let padded = decoder
            .decode_packet(&synthetic_silent_mp3_frame(true), 1152)
            .unwrap();

        // 1,152 frames * two channels * two bytes per i16 sample.
        assert_eq!(unpadded.len(), 4608);
        assert_eq!(padded.len(), 4608);
        assert!(unpadded.iter().all(|&byte| byte == 0));
        assert!(padded.iter().all(|&byte| byte == 0));
    }

    #[test]
    fn streaming_mp3_decoder_reset_restores_initial_state() {
        let frame = synthetic_silent_mp3_frame(false);
        let mut decoder = StreamingMp3Decoder::new(44_100, 2).unwrap();

        let initially_decoded = decoder.decode_packet(&frame, 1152).unwrap();
        decoder
            .decode_packet(&synthetic_silent_mp3_frame(true), 1152)
            .unwrap();
        decoder.reset();
        let decoded_after_reset = decoder.decode_packet(&frame, 1152).unwrap();

        assert_eq!(decoded_after_reset, initially_decoded);
    }
}
