//! Artwork decoding is isolated because image::Limits::max_alloc is best effort.
use super::{
    FormatError, FormatErrorKind as K, FormatResult, IMAGE_MEMORY_BUDGET, MAX_ARTWORK_BYTES,
    MAX_IMAGE_SIDE, check_dimensions,
};
use image::{ImageDecoder, ImageReader};
use std::{
    io::{Cursor, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    pub format: String,
    pub width: u32,
    pub height: u32,
}

pub(super) fn worker_path(name: &str, override_var: &str) -> FormatResult<PathBuf> {
    if let Some(path) = std::env::var_os(override_var) {
        return Ok(path.into());
    }
    let exe = std::env::current_exe()?;
    let parent = exe
        .parent()
        .ok_or_else(|| FormatError::new(K::ResourceLimit, "cannot locate image worker"))?;
    let directory = if parent.file_name().is_some_and(|n| n == "deps") {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };
    Ok(directory.join(name))
}

pub fn inspect_artwork(bytes: &[u8]) -> FormatResult<ImageInfo> {
    if bytes.len() as u64 > MAX_ARTWORK_BYTES {
        return Err(FormatError::new(
            K::ResourceLimit,
            "encoded artwork exceeds 20 MiB",
        ));
    }
    let mut child=Command::new(worker_path("vdisc-image-check","VDISC_IMAGE_WORKER")?).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|e|FormatError::new(K::ResourceLimit,format!("image worker unavailable: {e}; build/install vdisc-image-check beside the application")))?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| FormatError::new(K::Io, "worker stdin missing"))?
        .write_all(bytes);
    let mut response = Vec::new();
    let read_result = child
        .stdout
        .take()
        .ok_or_else(|| FormatError::new(K::Io, "worker stdout missing"))?
        .take(10)
        .read_to_end(&mut response);
    let status = child.wait()?;
    if !status.success() {
        let kind = if status.code() == Some(2) {
            K::InvalidArtwork
        } else {
            K::ResourceLimit
        };
        return Err(FormatError::new(
            kind,
            "image worker rejected artwork or exceeded its resource budget",
        ));
    }
    write_result?;
    read_result?;
    if response.len() != 9 {
        return Err(FormatError::new(
            K::InvalidArtwork,
            "invalid image worker response",
        ));
    }
    let format = match response[0] {
        1 => "png",
        2 => "jpeg",
        3 => "webp",
        _ => {
            return Err(FormatError::new(
                K::InvalidArtwork,
                "unsupported artwork format",
            ));
        }
    };
    let width = u32::from_le_bytes(response[1..5].try_into().expect("response length checked"));
    let height = u32::from_le_bytes(response[5..9].try_into().expect("response length checked"));
    check_dimensions(width, height)?;
    Ok(ImageInfo {
        format: format.to_owned(),
        width,
        height,
    })
}

/// Internal helper protocol entry point. Run only in the dedicated worker process.
#[doc(hidden)]
pub fn image_worker_main() -> i32 {
    if !worker_limits() {
        return 3;
    }
    {
        let mut bytes = Vec::new();
        if std::io::stdin()
            .take(MAX_ARTWORK_BYTES + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return 3;
        }
        if bytes.len() as u64 > MAX_ARTWORK_BYTES {
            return 3;
        }
        match decode_image(&bytes) {
            Ok(info) => {
                let kind = match info.format.as_str() {
                    "png" => 1,
                    "jpeg" => 2,
                    "webp" => 3,
                    _ => return 2,
                };
                let mut output = vec![kind];
                output.extend(info.width.to_le_bytes());
                output.extend(info.height.to_le_bytes());
                if std::io::stdout().write_all(&output).is_err() {
                    3
                } else {
                    0
                }
            }
            Err(e) => {
                if e.kind == K::ResourceLimit {
                    3
                } else {
                    2
                }
            }
        }
    }
}

// image::Limits is non-exhaustive, so external struct-update construction is unavailable.
#[allow(clippy::field_reassign_with_default)]
fn decode_image(bytes: &[u8]) -> FormatResult<ImageInfo> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = match reader.format() {
        Some(image::ImageFormat::Png) => "png",
        Some(image::ImageFormat::Jpeg) => "jpeg",
        Some(image::ImageFormat::WebP) => "webp",
        _ => {
            return Err(FormatError::new(
                K::InvalidArtwork,
                "unsupported image format",
            ));
        }
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(IMAGE_MEMORY_BUDGET);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    if decoder.total_bytes() > IMAGE_MEMORY_BUDGET {
        return Err(FormatError::new(
            K::ResourceLimit,
            "decoded image exceeds allocation budget",
        ));
    }
    image::DynamicImage::from_decoder(decoder).map_err(image_error)?;
    Ok(ImageInfo {
        format: format.to_owned(),
        width,
        height,
    })
}
fn image_error(e: image::ImageError) -> FormatError {
    let kind = if matches!(e, image::ImageError::Limits(_)) {
        K::ResourceLimit
    } else {
        K::InvalidArtwork
    };
    FormatError::new(kind, e.to_string())
}

pub(super) fn worker_limits() -> bool {
    #[cfg(target_os = "linux")]
    {
        let memory = libc::rlimit {
            rlim_cur: IMAGE_MEMORY_BUDGET as libc::rlim_t,
            rlim_max: IMAGE_MEMORY_BUDGET as libc::rlim_t,
        };
        let cpu = libc::rlimit {
            rlim_cur: 10,
            rlim_max: 10,
        };
        // SAFETY: both pointers refer to live rlimit values in a dedicated process.
        unsafe {
            libc::setrlimit(libc::RLIMIT_AS, &memory) == 0
                && libc::setrlimit(libc::RLIMIT_CPU, &cpu) == 0
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}
