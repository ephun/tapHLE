/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CommonCrypto and friends

use crate::dyld::FunctionExports;
use crate::mem::{ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::{export_c_func, Environment};
use aes::cipher::{Block, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use digest::Digest;
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512};
use std::collections::HashMap;

/// Host-side state for the streaming digest APIs.
///
/// `CC_MD5_CTX` is a real struct the guest declares itself, usually on the
/// stack, and the real CommonCrypto keeps the running state inside it. tapHLE
/// cannot do that — the [Md5] hasher's layout is not the same — so the guest's
/// context is used only as a key and the hasher lives here. That is sound
/// because a context is always initialised before use and always consumed by
/// `_Final`; what it does not survive is the guest copying a context by value
/// to fork a digest, which nothing does in practice and which would be
/// detected as a missing key rather than silently producing a wrong hash.
#[derive(Default)]
pub struct State {
    md5: HashMap<MutVoidPtr, Md5>,
}
impl State {
    fn get(env: &mut Environment) -> &mut Self {
        &mut env.libc_state.crypto
    }
}

type CCCryptorStatus = i32;

const KCC_SUCCESS: CCCryptorStatus = 0;
const KCC_PARAM_ERROR: CCCryptorStatus = -4300;
const KCC_BUFFER_TOO_SMALL: CCCryptorStatus = -4301;
const KCC_ALIGNMENT_ERROR: CCCryptorStatus = -4303;
const KCC_DECODE_ERROR: CCCryptorStatus = -4304;
const KCC_UNIMPLEMENTED: CCCryptorStatus = -4305;
const KCC_KEY_SIZE_ERROR: CCCryptorStatus = -4310;

const KCC_ENCRYPT: u32 = 0;
const KCC_DECRYPT: u32 = 1;
const KCC_ALGORITHM_AES128: u32 = 0;
const KCC_OPTION_PKCS7_PADDING: u32 = 1;
const KCC_OPTION_ECB_MODE: u32 = 2;
const AES_BLOCK_SIZE: usize = 16;
const CCHMAC_ALGORITHM_SHA1: u32 = 0;
const CCHMAC_ALGORITHM_MD5: u32 = 1;
const HMAC_BLOCK_SIZE: usize = 64;

fn CC_MD5(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    let mut hasher = Md5::new();
    hasher.update(env.mem.bytes_at(data.cast(), len));
    let digest = hasher.finalize();
    env.mem.bytes_at_mut(md, 16).copy_from_slice(&digest[..]);
    md
}

/// The streaming form of the above. All three return 1 for success, which is
/// what CommonCrypto does; they have no failure case here.
fn CC_MD5_Init(env: &mut Environment, ctx: MutVoidPtr) -> i32 {
    State::get(env).md5.insert(ctx, Md5::new());
    1
}

fn CC_MD5_Update(env: &mut Environment, ctx: MutVoidPtr, data: ConstVoidPtr, len: u32) -> i32 {
    let bytes = env.mem.bytes_at(data.cast(), len).to_vec();
    let Some(hasher) = State::get(env).md5.get_mut(&ctx) else {
        log!("CC_MD5_Update() on a context that was never initialised, ignoring");
        return 0;
    };
    hasher.update(&bytes);
    1
}

fn CC_MD5_Final(env: &mut Environment, md: MutPtr<u8>, ctx: MutVoidPtr) -> i32 {
    let Some(hasher) = State::get(env).md5.remove(&ctx) else {
        log!("CC_MD5_Final() on a context that was never initialised, ignoring");
        return 0;
    };
    let digest = hasher.finalize();
    env.mem.bytes_at_mut(md, 16).copy_from_slice(&digest[..]);
    1
}

fn CC_SHA1(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    let mut hasher = Sha1::new();
    hasher.update(env.mem.bytes_at(data.cast(), len));
    let digest = hasher.finalize();
    env.mem.bytes_at_mut(md, 20).copy_from_slice(&digest[..]);
    md
}

/// Shared one-shot body for the CC_SHA2 family. `D` is the digest type and its
/// output length determines how many bytes are written to `md`.
fn cc_sha2<D: Digest>(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) {
    let mut hasher = D::new();
    hasher.update(env.mem.bytes_at(data.cast(), len));
    let digest = hasher.finalize();
    let out_len = digest.len() as u32;
    env.mem
        .bytes_at_mut(md, out_len)
        .copy_from_slice(&digest[..]);
}

fn CC_SHA224(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    cc_sha2::<Sha224>(env, data, len, md);
    md
}
fn CC_SHA256(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    cc_sha2::<Sha256>(env, data, len, md);
    md
}
fn CC_SHA384(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    cc_sha2::<Sha384>(env, data, len, md);
    md
}
fn CC_SHA512(env: &mut Environment, data: ConstVoidPtr, len: u32, md: MutPtr<u8>) -> MutPtr<u8> {
    cc_sha2::<Sha512>(env, data, len, md);
    md
}

/// One-shot CommonCrypto HMAC API for the algorithms available on early
/// iPhone OS releases.
fn CCHmac(
    env: &mut Environment,
    algorithm: u32,
    key: ConstVoidPtr,
    key_length: GuestUSize,
    data: ConstVoidPtr,
    data_length: GuestUSize,
    mac_out: MutVoidPtr,
) {
    if mac_out.is_null()
        || (key_length != 0 && key.is_null())
        || (data_length != 0 && data.is_null())
    {
        log!("CCHmac received an invalid buffer");
        return;
    }

    let key = env.mem.bytes_at(key.cast(), key_length);
    let data = env.mem.bytes_at(data.cast(), data_length);
    let mac = match algorithm {
        CCHMAC_ALGORITHM_SHA1 => cchmac::<Sha1>(key, data),
        CCHMAC_ALGORITHM_MD5 => cchmac::<Md5>(key, data),
        _ => {
            log!("CCHmac({algorithm}, ...) is unsupported");
            return;
        }
    };
    env.mem
        .bytes_at_mut(mac_out.cast(), mac.len().try_into().unwrap())
        .copy_from_slice(&mac);
}

/// HMAC construction shared by the CommonCrypto wrapper and tests.
fn cchmac<D: Digest + Default>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut key_block = [0u8; HMAC_BLOCK_SIZE];
    if key.len() > HMAC_BLOCK_SIZE {
        let digest = D::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner = D::new();
    for byte in key_block {
        inner.update([byte ^ 0x36]);
    }
    inner.update(data);
    let inner_digest = inner.finalize();

    let mut outer = D::new();
    for byte in key_block {
        outer.update([byte ^ 0x5c]);
    }
    outer.update(inner_digest);
    outer.finalize().to_vec()
}

/// One-shot CommonCrypto block cipher API.
///
/// The early iPhone OS CommonCrypto manual documents AES in CBC mode by
/// default, a zero IV when none is supplied, PKCS#7 padding, and the ECB-mode
/// option. See <https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/CCryptorCreateFromData.3cc.html>.
// Twelve arguments because that is CommonCrypto's signature; the guest calls
// this by ABI, so the parameter list is not ours to group.
#[allow(clippy::too_many_arguments)]
fn CCCrypt(
    env: &mut Environment,
    operation: u32,
    algorithm: u32,
    options: u32,
    key: ConstVoidPtr,
    key_length: GuestUSize,
    iv: ConstVoidPtr,
    data_in: ConstVoidPtr,
    data_in_length: GuestUSize,
    data_out: MutVoidPtr,
    data_out_available: GuestUSize,
    data_out_moved: MutPtr<GuestUSize>,
) -> CCCryptorStatus {
    if !data_out_moved.is_null() {
        env.mem.write(data_out_moved, 0);
    }

    if algorithm != KCC_ALGORITHM_AES128 {
        log!(
            "CCCrypt({}, {}, {:#x}, ...) is unsupported: only AES is implemented",
            operation,
            algorithm,
            options,
        );
        return KCC_UNIMPLEMENTED;
    }
    if key.is_null() || (data_in_length != 0 && data_in.is_null()) {
        return KCC_PARAM_ERROR;
    }

    let key = env.mem.bytes_at(key.cast(), key_length);
    let data_in = env.mem.bytes_at(data_in.cast(), data_in_length);
    let iv = if iv.is_null() {
        None
    } else {
        Some(
            env.mem
                .bytes_at(iv.cast(), AES_BLOCK_SIZE.try_into().unwrap()),
        )
    };
    let output = match cccrypt_aes(operation, options, key, iv, data_in) {
        Ok(output) => output,
        Err(status) => return status,
    };

    if output.len() > data_out_available.try_into().unwrap() {
        return KCC_BUFFER_TOO_SMALL;
    }
    if !output.is_empty() {
        if data_out.is_null() {
            return KCC_PARAM_ERROR;
        }
        env.mem
            .bytes_at_mut(data_out.cast(), output.len().try_into().unwrap())
            .copy_from_slice(&output);
    }
    if !data_out_moved.is_null() {
        env.mem
            .write(data_out_moved, output.len().try_into().unwrap());
    }
    KCC_SUCCESS
}

/// AES implementation shared by the guest ABI wrapper and synthetic tests.
fn cccrypt_aes(
    operation: u32,
    options: u32,
    key: &[u8],
    iv: Option<&[u8]>,
    data_in: &[u8],
) -> Result<Vec<u8>, CCCryptorStatus> {
    let encrypting = match operation {
        KCC_ENCRYPT => true,
        KCC_DECRYPT => false,
        _ => return Err(KCC_PARAM_ERROR),
    };
    if options & !(KCC_OPTION_PKCS7_PADDING | KCC_OPTION_ECB_MODE) != 0 {
        return Err(KCC_PARAM_ERROR);
    }
    let padding = options & KCC_OPTION_PKCS7_PADDING != 0;
    let ecb = options & KCC_OPTION_ECB_MODE != 0;

    let mut output = data_in.to_vec();
    if encrypting && padding {
        let padding_len = AES_BLOCK_SIZE - output.len() % AES_BLOCK_SIZE;
        output.resize(output.len() + padding_len, padding_len as u8);
    } else if !output.len().is_multiple_of(AES_BLOCK_SIZE) {
        return Err(KCC_ALIGNMENT_ERROR);
    }

    let mut chain = [0u8; AES_BLOCK_SIZE];
    if !ecb {
        if let Some(iv) = iv {
            chain.copy_from_slice(iv);
        }
    }

    macro_rules! crypt_blocks {
        ($cipher:ty) => {{
            let cipher = <$cipher>::new_from_slice(key).unwrap();
            for block in output.chunks_exact_mut(AES_BLOCK_SIZE) {
                let block: &mut [u8; AES_BLOCK_SIZE] = block.try_into().unwrap();
                if encrypting {
                    if !ecb {
                        xor_block(block, &chain);
                    }
                    cipher.encrypt_block(Block::<$cipher>::from_mut_slice(block));
                    if !ecb {
                        chain.copy_from_slice(block);
                    }
                } else {
                    let ciphertext = *block;
                    cipher.decrypt_block(Block::<$cipher>::from_mut_slice(block));
                    if !ecb {
                        xor_block(block, &chain);
                        chain = ciphertext;
                    }
                }
            }
        }};
    }

    match key.len() {
        16 => crypt_blocks!(Aes128),
        24 => crypt_blocks!(Aes192),
        32 => crypt_blocks!(Aes256),
        _ => return Err(KCC_KEY_SIZE_ERROR),
    }

    if !encrypting && padding {
        let Some(&padding_len) = output.last() else {
            return Err(KCC_DECODE_ERROR);
        };
        let padding_len = usize::from(padding_len);
        if !(1..=AES_BLOCK_SIZE).contains(&padding_len)
            || output[output.len() - padding_len..]
                .iter()
                .any(|&byte| usize::from(byte) != padding_len)
        {
            return Err(KCC_DECODE_ERROR);
        }
        output.truncate(output.len() - padding_len);
    }

    Ok(output)
}

fn xor_block(block: &mut [u8; AES_BLOCK_SIZE], chain: &[u8; AES_BLOCK_SIZE]) {
    for (byte, chain_byte) in block.iter_mut().zip(chain) {
        *byte ^= chain_byte;
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CC_MD5(_, _, _)),
    export_c_func!(CC_MD5_Init(_)),
    export_c_func!(CC_MD5_Update(_, _, _)),
    export_c_func!(CC_MD5_Final(_, _)),
    export_c_func!(CC_SHA1(_, _, _)),
    export_c_func!(CC_SHA224(_, _, _)),
    export_c_func!(CC_SHA256(_, _, _)),
    export_c_func!(CC_SHA384(_, _, _)),
    export_c_func!(CC_SHA512(_, _, _)),
    export_c_func!(CCHmac(_, _, _, _, _, _)),
    export_c_func!(CCCrypt(_, _, _, _, _, _, _, _, _, _, _)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cchmac_matches_rfc_2202_vectors() {
        let key = [0x0b; 20];
        assert_eq!(
            cchmac::<Sha1>(&key, b"Hi There"),
            [
                0xb6, 0x17, 0x31, 0x86, 0x55, 0x05, 0x72, 0x64, 0xe2, 0x8b, 0xc0, 0xb6, 0xfb, 0x37,
                0x8c, 0x8e, 0xf1, 0x46, 0xbe, 0x00,
            ]
        );
        assert_eq!(
            cchmac::<Md5>(&[0x0b; 16], b"Hi There"),
            [
                0x92, 0x94, 0x72, 0x7a, 0x36, 0x38, 0xbb, 0x1c, 0x13, 0xf4, 0x8e, 0xf8, 0x15, 0x8b,
                0xfc, 0x9d,
            ]
        );
    }

    #[test]
    fn cccrypt_aes128_ecb_matches_nist_vector() {
        let key = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let plaintext = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        let expected_ciphertext = [
            0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60, 0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66,
            0xef, 0x97,
        ];

        let ciphertext =
            cccrypt_aes(KCC_ENCRYPT, KCC_OPTION_ECB_MODE, &key, None, &plaintext).unwrap();
        assert_eq!(ciphertext, expected_ciphertext);
        assert_eq!(
            cccrypt_aes(KCC_DECRYPT, KCC_OPTION_ECB_MODE, &key, None, &ciphertext).unwrap(),
            plaintext,
        );
    }

    #[test]
    fn cccrypt_aes128_cbc_matches_nist_vector() {
        let key = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plaintext = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        let expected_ciphertext = [
            0x76, 0x49, 0xab, 0xac, 0x81, 0x19, 0xb2, 0x46, 0xce, 0xe9, 0x8e, 0x9b, 0x12, 0xe9,
            0x19, 0x7d,
        ];

        let ciphertext = cccrypt_aes(KCC_ENCRYPT, 0, &key, Some(&iv), &plaintext).unwrap();
        assert_eq!(ciphertext, expected_ciphertext);
        assert_eq!(
            cccrypt_aes(KCC_DECRYPT, 0, &key, Some(&iv), &ciphertext).unwrap(),
            plaintext
        );
    }

    #[test]
    fn cccrypt_pkcs7_round_trips_and_rejects_bad_padding() {
        let key = [0x5a; AES_BLOCK_SIZE];
        let plaintext = b"tapHLE test";
        let ciphertext =
            cccrypt_aes(KCC_ENCRYPT, KCC_OPTION_PKCS7_PADDING, &key, None, plaintext).unwrap();
        assert_eq!(
            cccrypt_aes(
                KCC_DECRYPT,
                KCC_OPTION_PKCS7_PADDING,
                &key,
                None,
                &ciphertext
            )
            .unwrap(),
            plaintext
        );

        let mut malformed = ciphertext;
        *malformed.last_mut().unwrap() ^= 1;
        assert_eq!(
            cccrypt_aes(
                KCC_DECRYPT,
                KCC_OPTION_PKCS7_PADDING,
                &key,
                None,
                &malformed
            ),
            Err(KCC_DECODE_ERROR)
        );
    }
}
