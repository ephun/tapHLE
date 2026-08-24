/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Narrow fallback for AIFF-C containers holding Apple IMA4 ADPCM.
//!
//! `afconvert -f AIFC -d ima4` is what Apple's own tooling produced for game
//! audio of this era, so it is a common shape: a game can easily ship every
//! one of its several hundred sounds this way, from menu clicks to the
//! voice-over an opening cutscene waits on. Symphonia reads AIFF but rejects
//! the compressed AIFC variant, so none of them decoded and the app
//! stalled in its introduction waiting for narration that could never play.
//!
//! tapHLE already decodes IMA4 — see [super::ima4] — but only at playback, once
//! an audio queue is running with `kAudioFormatAppleIMA4`. Nothing could read
//! the container to get that far. This does the same job the CAF fallback does
//! next door: recognise the container, decode it to the 16-bit little-endian
//! PCM the rest of the decoded-file path expects, and stay narrow.
//!
//! Resources:
//! - [Audio Interchange File Format AIFF-C](https://web.archive.org/web/20071219035439/http://www.cnpbagwell.com/aiff-c.txt)

use super::ima4::decode_ima4;
use super::symphonia_formats::SymphoniaDecodedToPcm;

const FORM_HEADER_SIZE: usize = 12;
const CHUNK_HEADER_SIZE: usize = 8;
/// An IMA4 packet is 34 bytes of one channel and expands to 64 samples.
const IMA4_PACKET_SIZE: usize = 34;
const IMA4_SAMPLES_PER_PACKET: usize = 64;

/// Read an 80-bit IEEE 754 extended float, which is how AIFF stores the sample
/// rate and the only place this format uses the type.
///
/// Rates are small positive integers in practice, so this only needs the
/// normalised range: reject anything else rather than approximate it.
fn read_extended_80(bytes: &[u8]) -> Result<u32, ()> {
    let sign_and_exponent = u16::from_be_bytes(bytes[..2].try_into().unwrap());
    let mantissa = u64::from_be_bytes(bytes[2..10].try_into().unwrap());
    if sign_and_exponent & 0x8000 != 0 {
        return Err(()); // negative
    }
    let exponent = i32::from(sign_and_exponent & 0x7FFF) - 16383;
    // The leading integer bit is explicit in this format, unlike IEEE binary64.
    if !(0..=63).contains(&exponent) {
        return Err(());
    }
    let shift = 63 - exponent;
    u32::try_from(mantissa >> shift).map_err(|_| ())
}

/// Decode Apple IMA4 ADPCM from an AIFF-C container.
pub fn decode_ima4_aifc(bytes: &[u8]) -> Result<SymphoniaDecodedToPcm, ()> {
    let header = bytes.get(..FORM_HEADER_SIZE).ok_or(())?;
    if &header[..4] != b"FORM" || &header[8..12] != b"AIFC" {
        return Err(());
    }

    let mut channels = None;
    let mut sample_rate = None;
    let mut sound_data = None;

    let mut position = FORM_HEADER_SIZE;
    while position + CHUNK_HEADER_SIZE <= bytes.len() {
        let id = &bytes[position..position + 4];
        let size = u32::from_be_bytes(bytes[position + 4..position + 8].try_into().unwrap());
        let size = usize::try_from(size).map_err(|_| ())?;
        let body_start = position + CHUNK_HEADER_SIZE;
        let body = bytes.get(body_start..body_start + size).ok_or(())?;

        match id {
            b"COMM" => {
                // numChannels, numSampleFrames, sampleSize, sampleRate(80-bit),
                // then the AIFC compression FourCC and a Pascal-string name.
                if size < 22 {
                    return Err(());
                }
                if &body[18..22] != b"ima4" {
                    // A different compression: not ours to decode. Symphonia
                    // handles uncompressed AIFF, so let it try.
                    return Err(());
                }
                channels = Some(u32::from(u16::from_be_bytes(body[..2].try_into().unwrap())));
                sample_rate = Some(read_extended_80(&body[8..18])?);
            }
            b"SSND" => {
                // offset and blockSize precede the samples.
                if size < 8 {
                    return Err(());
                }
                let offset = u32::from_be_bytes(body[..4].try_into().unwrap());
                let offset = usize::try_from(offset).map_err(|_| ())?;
                sound_data = Some(body.get(8 + offset..).ok_or(())?);
            }
            _ => {}
        }

        // Chunks are padded to an even length, and the pad byte is not counted
        // in the size.
        position = body_start + size + (size & 1);
    }

    let channels = channels.ok_or(())?;
    let sample_rate = sample_rate.ok_or(())?;
    let sound_data = sound_data.ok_or(())?;
    if channels == 0 || sample_rate == 0 {
        return Err(());
    }
    let channels_usize = usize::try_from(channels).map_err(|_| ())?;

    // Packets are per-channel and interleaved by packet, not by sample: for
    // stereo the first is the whole left channel of a frame group and the
    // second is the whole right. Decode a group at a time so the output can be
    // interleaved per sample the way everything downstream expects.
    let group_size = IMA4_PACKET_SIZE * channels_usize;
    let groups = sound_data.len() / group_size;
    let mut out = Vec::with_capacity(groups * IMA4_SAMPLES_PER_PACKET * channels_usize * 2);
    let mut decoded = vec![[0i16; IMA4_SAMPLES_PER_PACKET]; channels_usize];
    for group in 0..groups {
        let group_start = group * group_size;
        for (channel, slot) in decoded.iter_mut().enumerate() {
            let start = group_start + channel * IMA4_PACKET_SIZE;
            let packet: &[u8; IMA4_PACKET_SIZE] = sound_data[start..start + IMA4_PACKET_SIZE]
                .try_into()
                .map_err(|_| ())?;
            *slot = decode_ima4(packet);
        }
        for sample in 0..IMA4_SAMPLES_PER_PACKET {
            for channel_samples in &decoded {
                out.extend_from_slice(&channel_samples[sample].to_le_bytes());
            }
        }
    }

    if out.is_empty() {
        return Err(());
    }

    Ok(SymphoniaDecodedToPcm {
        bytes: out,
        sample_rate,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_ima4_aifc, read_extended_80};

    /// The sample rates these files actually use.
    #[test]
    fn extended_floats_round_trip_common_rates() {
        // 22050 Hz and 44100 Hz as 80-bit extended.
        let rate_22050 = [0x40, 0x0D, 0xAC, 0x44, 0, 0, 0, 0, 0, 0];
        let rate_44100 = [0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0];
        assert_eq!(read_extended_80(&rate_22050), Ok(22050));
        assert_eq!(read_extended_80(&rate_44100), Ok(44100));
    }

    fn aifc_with(compression: &[u8; 4], packets: usize) -> Vec<u8> {
        let mut comm = Vec::new();
        comm.extend_from_slice(&1u16.to_be_bytes()); // channels
        comm.extend_from_slice(&(packets as u32 * 64).to_be_bytes()); // frames
        comm.extend_from_slice(&0u16.to_be_bytes()); // sampleSize (0 for AIFC)
        comm.extend_from_slice(&[0x40, 0x0D, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]); // 22050
        comm.extend_from_slice(compression);
        comm.push(0); // empty Pascal string

        let mut ssnd = vec![0u8; 8];
        ssnd.extend(std::iter::repeat_n(0u8, packets * 34));

        let mut body = Vec::new();
        body.extend_from_slice(b"AIFC");
        body.extend_from_slice(b"COMM");
        body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
        body.extend_from_slice(&comm);
        if comm.len() % 2 == 1 {
            body.push(0);
        }
        body.extend_from_slice(b"SSND");
        body.extend_from_slice(&(ssnd.len() as u32).to_be_bytes());
        body.extend_from_slice(&ssnd);

        let mut out = Vec::new();
        out.extend_from_slice(b"FORM");
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn an_ima4_aifc_decodes_to_sixteen_bit_pcm() {
        let decoded = decode_ima4_aifc(&aifc_with(b"ima4", 3)).unwrap();
        assert_eq!(decoded.sample_rate, 22050);
        assert_eq!(decoded.channels, 1);
        // Three packets, 64 samples each, two bytes per sample.
        assert_eq!(decoded.bytes.len(), 3 * 64 * 2);
    }

    /// Uncompressed AIFF and other compressions belong to Symphonia; refusing
    /// them here is what lets it still get a look.
    #[test]
    fn other_compressions_are_declined() {
        assert!(decode_ima4_aifc(&aifc_with(b"NONE", 1)).is_err());
        assert!(decode_ima4_aifc(&aifc_with(b"alaw", 1)).is_err());
    }

    #[test]
    fn a_non_aifc_file_is_declined() {
        assert!(decode_ima4_aifc(b"RIFF\0\0\0\0WAVEfmt ").is_err());
        assert!(decode_ima4_aifc(b"").is_err());
    }
}
