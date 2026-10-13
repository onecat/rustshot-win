//! Tiny, dependency-free PNG writer (zlib DEFLATE stored blocks).
//! PoC output is deliberately lossless but **not** compressed; optimize in P3.
use std::io::{self, Write};

pub fn encode_bgra<W: Write>(mut out: W, width: u32, height: u32, bgra: &[u8]) -> io::Result<()> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|w| w.checked_mul(usize::try_from(height).ok()?))
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "image size overflow"))?;
    if width == 0 || height == 0 || bgra.len() != expected {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid BGRA image"));
    }

    out.write_all(b"\x89PNG\r\n\x1a\n")?;
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    write_chunk(&mut out, b"IHDR", &ihdr)?;

    // An entire raw scanline has a preceding filter byte 0; stream the zlib payload.
    let line_bytes = (width as usize).checked_mul(3).ok_or_else(|| io::Error::other("overflow"))?;
    let total = (line_bytes + 1).checked_mul(height as usize).ok_or_else(|| io::Error::other("overflow"))?;
    let mut raw = Vec::with_capacity(total);
    for row in bgra.chunks_exact(width as usize * 4) {
        raw.push(0);
        for px in row.chunks_exact(4) {
            raw.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    let compressed = zlib_stored(&raw);
    // Split into IDAT chunks for reasonable buffer sizes.
    for segment in compressed.chunks(1024 * 1024) {
        write_chunk(&mut out, b"IDAT", segment)?;
    }
    write_chunk(&mut out, b"IEND", &[])?;
    Ok(())
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + (data.len() / 65_535 + 1) * 5 + 6);
    out.extend_from_slice(&[0x78, 0x01]); // zlib header: DEFLATE, fastest compression
    for (i, chunk) in data.chunks(65_535).enumerate() {
        let last = (i + 1) * 65_535 >= data.len();
        out.push(if last { 1 } else { 0 }); // BFINAL, stored block, byte aligned
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &value in bytes {
        crc ^= u32::from(value);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn write_chunk<W: Write>(out: &mut W, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    let len = u32::try_from(data.len()).map_err(|_| io::Error::other("PNG chunk too large"))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let mut check = Vec::with_capacity(4 + data.len());
    check.extend_from_slice(kind);
    check.extend_from_slice(data);
    out.write_all(&crc32(&check).to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_signature_and_dimensions() {
        let mut bytes = Vec::new();
        encode_bgra(&mut bytes, 2, 1, &[0, 0, 255, 255, 0, 255, 0, 255]).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[16..24], &[0, 0, 0, 2, 0, 0, 0, 1]);
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
    }
    #[test]
    fn reject_bad_image() {
        assert!(encode_bgra(Vec::new(), 5, 5, &[0; 4]).is_err());
        assert!(encode_bgra(Vec::new(), u32::MAX, u32::MAX, &[]).is_err());
    }
    #[test]
    fn checksum_known_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11e60398);
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }
}
