/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Debugging utility functions. These are "dead code" that you can use in
//! debug hacks.
#![allow(dead_code)]

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Dump RGB8 pixel data to a file in PPM format.
pub fn write_ppm(path: &str, width: u32, height: u32, pixels: &[u8]) {
    use std::io::Write;

    let mut file = File::create(path).unwrap();
    writeln!(file, "P6 {width} {height} 255").unwrap();
    file.write_all(pixels).unwrap();
}

/// Create a new PPM file without replacing an existing file.
///
/// Unlike [`write_ppm`], this is suitable for optional diagnostics: all I/O
/// errors are returned to the caller and a partially written file is removed
/// on a best-effort basis.
pub fn write_ppm_create_new(path: &Path, width: u32, height: u32, pixels: &[u8]) -> io::Result<()> {
    use std::io::Write;

    if width == 0 || height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PPM dimensions must be non-zero",
        ));
    }

    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|size| size.checked_mul(3))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "PPM dimensions overflow"))?;
    if pixels.len() != expected_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "PPM pixel payload has {} bytes, expected {expected_len}",
                pixels.len()
            ),
        ));
    }

    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let write_result = (|| {
        writeln!(file, "P6 {width} {height} 255")?;
        file.write_all(pixels)?;
        file.flush()
    })();

    if let Err(err) = write_result {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(err);
    }

    Ok(())
}

/// Convert RGBA8 pixel data to RGB8 pixel data by discarding alpha component.
/// Useful with [write_ppm] for example.
pub fn rgba8_to_rgb8(pixels: &[u8]) -> Vec<u8> {
    assert!(pixels.len().is_multiple_of(4));
    let mut res = Vec::with_capacity((pixels.len() / 4) * 3);
    for rgba in pixels.chunks(4) {
        res.extend_from_slice(&rgba[..3]);
    }
    res
}

/// Dump a region of the current OpenGL ES framebuffer to a file.
pub fn dump_framebuffer(
    path: &str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    gles: &mut dyn crate::gles::GLES,
) {
    let mut rgba8_pixels = Vec::<u8>::with_capacity(width as usize * height as usize * 4);
    // 0x7F grey chosen to make missing data obvious: it's more likely to
    // contrast with typical backgrounds like black or white
    rgba8_pixels.resize(rgba8_pixels.capacity(), 0x7F);
    unsafe {
        gles.ReadPixels(
            x.try_into().unwrap(),
            y.try_into().unwrap(),
            width.try_into().unwrap(),
            height.try_into().unwrap(),
            crate::gles::gles11_raw::RGBA,
            crate::gles::gles11_raw::UNSIGNED_BYTE,
            rgba8_pixels.as_mut_ptr() as *mut _,
        );
        rgba8_pixels.set_len(rgba8_pixels.capacity());
    }
    write_ppm(path, width, height, &rgba8_to_rgb8(&rgba8_pixels));
}

#[cfg(test)]
mod tests {
    use super::write_ppm_create_new;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_test_path(label: &str) -> std::path::PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "taphle-{label}-{}-{}.ppm",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn write_ppm_create_new_writes_header_and_pixels() {
        let path = unique_test_path("write-ppm");
        let pixels = [0, 1, 2, 3, 4, 5];

        write_ppm_create_new(&path, 2, 1, &pixels).unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(contents, [b"P6 2 1 255\n".as_slice(), &pixels].concat());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_ppm_create_new_preserves_existing_file() {
        let path = unique_test_path("preserve-ppm");
        let mut existing = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        existing.write_all(b"existing evidence").unwrap();
        drop(existing);

        let err = write_ppm_create_new(&path, 1, 1, &[1, 2, 3]).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), b"existing evidence");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn write_ppm_create_new_rejects_invalid_dimensions_and_payload() {
        let path = unique_test_path("reject-ppm");

        let dimensions_err = write_ppm_create_new(&path, 0, 1, &[]).unwrap_err();
        assert_eq!(dimensions_err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!path.exists());

        let payload_err = write_ppm_create_new(&path, 1, 1, &[1, 2]).unwrap_err();
        assert_eq!(payload_err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!path.exists());
    }
}
