/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Narrow fallbacks for Core Audio Format containers that Symphonia cannot
//! currently decode.

use super::symphonia_formats::SymphoniaDecodedToPcm;

const CAF_HEADER_SIZE: usize = 8;
const CHUNK_HEADER_SIZE: usize = 12;
const AUDIO_DESCRIPTION_SIZE: usize = 32;
// These serialized CAF-file flags intentionally differ from the similarly
// named AudioStreamBasicDescription flags in CoreAudioTypes.
const CAF_LPCM_FLAG_IS_FLOAT: u32 = 1 << 0;
const CAF_LPCM_FLAG_IS_LITTLE_ENDIAN: u32 = 1 << 1;
const CAF_LPCM_KNOWN_FLAGS: u32 = CAF_LPCM_FLAG_IS_FLOAT | CAF_LPCM_FLAG_IS_LITTLE_ENDIAN;

struct Signed8BitDescription {
    sample_rate: u32,
    channels: u32,
}

/// Decode signed 8-bit interleaved LPCM from a version 1 CAF container.
///
/// Symphonia 0.6 supports signed 8-bit PCM itself, but its CAF reader rejects
/// that bit depth before exposing a track. Keep this fallback deliberately
/// narrow and convert its output to the same 16-bit little-endian shape used
/// by the rest of tapHLE's decoded-file path.
pub fn decode_signed_8_bit_lpcm(bytes: &[u8]) -> Result<SymphoniaDecodedToPcm, ()> {
    let header = bytes.get(..CAF_HEADER_SIZE).ok_or(())?;
    if &header[..4] != b"caff"
        || u16::from_be_bytes(header[4..6].try_into().unwrap()) != 1
        || u16::from_be_bytes(header[6..8].try_into().unwrap()) != 0
    {
        return Err(());
    }

    let mut description = None;
    let mut audio_data = None;
    let mut position = CAF_HEADER_SIZE;

    while position < bytes.len() {
        let chunk_header = bytes
            .get(position..position.checked_add(CHUNK_HEADER_SIZE).ok_or(())?)
            .ok_or(())?;
        let chunk_type: [u8; 4] = chunk_header[..4].try_into().unwrap();
        let chunk_size = i64::from_be_bytes(chunk_header[4..12].try_into().unwrap());
        position += CHUNK_HEADER_SIZE;

        let (chunk, next_position) = if chunk_size == -1 {
            if &chunk_type != b"data" {
                return Err(());
            }
            (&bytes[position..], bytes.len())
        } else {
            let chunk_size: usize = chunk_size.try_into().map_err(|_| ())?;
            let end = position.checked_add(chunk_size).ok_or(())?;
            (bytes.get(position..end).ok_or(())?, end)
        };

        match &chunk_type {
            b"desc" => {
                if description.is_some() {
                    return Err(());
                }
                description = Some(parse_signed_8_bit_description(chunk)?);
            }
            b"data" => {
                if audio_data.is_some() || description.is_none() {
                    return Err(());
                }
                if chunk.len() < 4 {
                    return Err(());
                }
                // The edit count is metadata describing the data history. It
                // may be nonzero in otherwise ordinary PCM CAF assets, and
                // does not change the sample payload layout.
                let (_edit_count, samples) = chunk.split_at(4);
                audio_data = Some(samples);
            }
            _ => {}
        }

        position = next_position;
        if chunk_size == -1 {
            break;
        }
    }

    let description = description.ok_or(())?;
    let audio_data = audio_data.filter(|data| !data.is_empty()).ok_or(())?;
    let channels: usize = description.channels.try_into().map_err(|_| ())?;
    if !audio_data.len().is_multiple_of(channels) {
        return Err(());
    }

    let mut decoded = Vec::with_capacity(audio_data.len().checked_mul(2).ok_or(())?);
    for &sample in audio_data {
        let sample = i16::from(sample as i8) << 8;
        decoded.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(SymphoniaDecodedToPcm {
        bytes: decoded,
        sample_rate: description.sample_rate,
        channels: description.channels,
    })
}

fn parse_signed_8_bit_description(chunk: &[u8]) -> Result<Signed8BitDescription, ()> {
    if chunk.len() != AUDIO_DESCRIPTION_SIZE || &chunk[8..12] != b"lpcm" {
        return Err(());
    }

    let sample_rate = f64::from_bits(u64::from_be_bytes(chunk[..8].try_into().unwrap()));
    if !sample_rate.is_finite()
        || sample_rate <= 0.0
        || sample_rate > f64::from(u32::MAX)
        || sample_rate.fract() != 0.0
    {
        return Err(());
    }

    let format_flags = u32::from_be_bytes(chunk[12..16].try_into().unwrap());
    let bytes_per_packet = u32::from_be_bytes(chunk[16..20].try_into().unwrap());
    let frames_per_packet = u32::from_be_bytes(chunk[20..24].try_into().unwrap());
    let channels = u32::from_be_bytes(chunk[24..28].try_into().unwrap());
    let bits_per_channel = u32::from_be_bytes(chunk[28..32].try_into().unwrap());

    // In the CAF file format, a clear float bit means signed integer. The
    // endianness bit has no effect on one-byte samples; all other serialized
    // CAF flags are reserved and outside this bounded fallback.
    if format_flags & CAF_LPCM_FLAG_IS_FLOAT != 0
        || format_flags & !CAF_LPCM_KNOWN_FLAGS != 0
        || !matches!(channels, 1 | 2)
        || bytes_per_packet != channels
        || frames_per_packet != 1
        || bits_per_channel != 8
    {
        return Err(());
    }

    Ok(Signed8BitDescription {
        sample_rate: sample_rate as u32,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        decode_signed_8_bit_lpcm, CAF_HEADER_SIZE, CAF_LPCM_FLAG_IS_LITTLE_ENDIAN,
        CHUNK_HEADER_SIZE,
    };

    fn synthetic_caf(
        channels: u32,
        samples: &[u8],
        unknown_data_size: bool,
        edit_count: u32,
    ) -> Vec<u8> {
        let mut bytes = b"caff".to_vec();
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&0_u16.to_be_bytes());

        let mut description = Vec::with_capacity(32);
        description.extend_from_slice(&48_000_f64.to_bits().to_be_bytes());
        description.extend_from_slice(b"lpcm");
        description.extend_from_slice(&CAF_LPCM_FLAG_IS_LITTLE_ENDIAN.to_be_bytes());
        description.extend_from_slice(&channels.to_be_bytes());
        description.extend_from_slice(&1_u32.to_be_bytes());
        description.extend_from_slice(&channels.to_be_bytes());
        description.extend_from_slice(&8_u32.to_be_bytes());
        push_chunk(&mut bytes, b"desc", &description);

        // Exercise bounded skipping of an unrelated chunk.
        push_chunk(&mut bytes, b"free", &[0, 0, 0, 0]);

        bytes.extend_from_slice(b"data");
        if unknown_data_size {
            bytes.extend_from_slice(&(-1_i64).to_be_bytes());
        } else {
            let data_size: i64 = (samples.len() + 4).try_into().unwrap();
            bytes.extend_from_slice(&data_size.to_be_bytes());
        }
        bytes.extend_from_slice(&edit_count.to_be_bytes());
        bytes.extend_from_slice(samples);
        bytes
    }

    fn push_chunk(file: &mut Vec<u8>, chunk_type: &[u8; 4], body: &[u8]) {
        file.extend_from_slice(chunk_type);
        file.extend_from_slice(&i64::try_from(body.len()).unwrap().to_be_bytes());
        file.extend_from_slice(body);
    }

    fn pcm_bytes(samples: &[i16]) -> Vec<u8> {
        samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect()
    }

    #[test]
    fn decodes_mono_signed_8_bit_lpcm() {
        let pcm =
            decode_signed_8_bit_lpcm(&synthetic_caf(1, &[0x80, 0x00, 0x7f], false, 0)).unwrap();

        assert_eq!(pcm.sample_rate, 48_000);
        assert_eq!(pcm.channels, 1);
        assert_eq!(pcm.bytes, pcm_bytes(&[i16::MIN, 0, 32_512]));
    }

    #[test]
    fn decodes_stereo_signed_8_bit_lpcm_with_unknown_data_size() {
        let pcm = decode_signed_8_bit_lpcm(&synthetic_caf(2, &[0x80, 0x7f, 0x00, 0xff], true, 0))
            .unwrap();

        assert_eq!(pcm.sample_rate, 48_000);
        assert_eq!(pcm.channels, 2);
        assert_eq!(pcm.bytes, pcm_bytes(&[i16::MIN, 32_512, 0, -256]));
    }

    #[test]
    fn accepts_nonzero_data_edit_count() {
        let pcm = decode_signed_8_bit_lpcm(&synthetic_caf(1, &[0x00], false, 1)).unwrap();
        assert_eq!(pcm.bytes, pcm_bytes(&[0]));
    }

    #[test]
    fn malformed_or_out_of_scope_caf_is_rejected_without_panicking() {
        let truncated = b"caff\0\x01\0\0desc".to_vec();
        let misaligned_stereo = synthetic_caf(2, &[0], false, 0);
        let mut unsupported_bit_depth = synthetic_caf(1, &[0], false, 0);
        let bits_per_channel_offset = CAF_HEADER_SIZE + CHUNK_HEADER_SIZE + 28;
        unsupported_bit_depth[bits_per_channel_offset..bits_per_channel_offset + 4]
            .copy_from_slice(&16_u32.to_be_bytes());
        let mut invalid_unknown_chunk = b"caff\0\x01\0\0free".to_vec();
        invalid_unknown_chunk.extend_from_slice(&(-1_i64).to_be_bytes());

        for malformed in [
            truncated,
            misaligned_stereo,
            unsupported_bit_depth,
            invalid_unknown_chunk,
        ] {
            assert!(decode_signed_8_bit_lpcm(&malformed).is_err());
        }
    }
}
