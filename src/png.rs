//! Bounded-memory, dependency-free PNG encoder.
//!
//! PNG IDAT chunks form one contiguous zlib stream. We emit RFC1951 stored
//! DEFLATE blocks in bounded chunks rather than materializing an RGB image and
//! a second compressed buffer. File size is intentionally not yet optimized.
use std::io::{self, Write};

const BLOCK: usize = 65_535;
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];

pub fn encode_bgra<W: Write>(mut out: W, width: u32, height: u32, bgra: &[u8]) -> io::Result<()> {
    let w = usize::try_from(width).map_err(|_| invalid("width overflow"))?;
    let h = usize::try_from(height).map_err(|_| invalid("height overflow"))?;
    let pixels = w.checked_mul(h).and_then(|n| n.checked_mul(4))
        .ok_or_else(|| invalid("image size overflow"))?;
    let scanline_bytes = w.checked_mul(3).and_then(|n| n.checked_add(1))
        .ok_or_else(|| invalid("scanline size overflow"))?;
    let total = scanline_bytes.checked_mul(h)
        .ok_or_else(|| invalid("encoded image size overflow"))?;
    if width == 0 || height == 0 || bgra.len() != pixels {
        return Err(invalid("invalid BGRA image"));
    }

    out.write_all(b"\x89PNG\r\n\x1a\n")?;
    let mut ihdr = [0u8; 13];
    ihdr[..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8..].copy_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    write_chunk(&mut out, b"IHDR", &ihdr)?;

    write_chunk(&mut out, b"IDAT", &ZLIB_HEADER)?;
    let mut zlib = StoredStream::new(&mut out, total);
    // An RGB scanline and a 65,535-byte staging block bound extra memory.
    // The full BGRA crop is owned by the caller for clipboard/export sharing.
    let mut line = vec![0u8; scanline_bytes];
    for row in bgra.chunks_exact(w * 4) {
        line[0] = 0; // PNG filter type None
        for (rgb, px) in line[1..].chunks_exact_mut(3).zip(row.chunks_exact(4)) {
            rgb.copy_from_slice(&[px[2], px[1], px[0]]);
        }
        zlib.write_input(&line)?;
    }
    zlib.finish()?;
    write_chunk(&mut out, b"IEND", &[])?;
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

struct StoredStream<'a, W: Write> {
    out: &'a mut W,
    pending: Vec<u8>,
    remaining: usize,
    adler_a: u32,
    adler_b: u32,
}

impl<'a, W: Write> StoredStream<'a, W> {
    fn new(out: &'a mut W, remaining: usize) -> Self {
        Self { out, pending: Vec::with_capacity(BLOCK), remaining, adler_a: 1, adler_b: 0 }
    }

    fn write_input(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > self.remaining { return Err(invalid("zlib input too long")); }
        while !bytes.is_empty() {
            let take = bytes.len().min(BLOCK - self.pending.len());
            let (part, rest) = bytes.split_at(take);
            self.update_adler(part);
            self.pending.extend_from_slice(part);
            self.remaining -= take;
            bytes = rest;
            if self.pending.len() == BLOCK { self.flush_block()?; }
        }
        Ok(())
    }

    fn update_adler(&mut self, bytes: &[u8]) {
        // Per-slice modulo ensures u32 cannot overflow for long screenshots.
        for chunk in bytes.chunks(5_552) {
            for &value in chunk {
                self.adler_a += u32::from(value);
                self.adler_b += self.adler_a;
            }
            self.adler_a %= 65_521;
            self.adler_b %= 65_521;
        }
    }

    fn flush_block(&mut self) -> io::Result<()> {
        if self.pending.is_empty() { return Ok(()); }
        let len = u16::try_from(self.pending.len()).map_err(|_| invalid("DEFLATE block size"))?;
        let mut payload = Vec::with_capacity(self.pending.len() + 5);
        // Since every stored DEFLATE block is byte-aligned, arbitrary IDAT
        // boundaries preserve the single deflate stream.
        payload.push(if self.remaining == 0 { 1 } else { 0 }); // BFINAL
        payload.extend_from_slice(&len.to_le_bytes());
        payload.extend_from_slice(&(!len).to_le_bytes());
        payload.extend_from_slice(&self.pending);
        write_chunk(self.out, b"IDAT", &payload)?;
        self.pending.clear();
        Ok(())
    }

    fn finish(mut self) -> io::Result<()> {
        if self.remaining != 0 { return Err(invalid("zlib input too short")); }
        self.flush_block()?;
        let adler = (self.adler_b << 16) | self.adler_a;
        write_chunk(self.out, b"IDAT", &adler.to_be_bytes())
    }
}

fn update_crc(mut crc: u32, bytes: &[u8]) -> u32 {
    for &value in bytes {
        crc ^= u32::from(value);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    crc
}

fn crc32(bytes: &[u8]) -> u32 {
    !update_crc(!0u32, bytes)
}

fn write_chunk<W: Write>(out: &mut W, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    let len = u32::try_from(data.len()).map_err(|_| invalid("PNG chunk too large"))?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let crc = !update_crc(update_crc(!0u32, kind), data);
    out.write_all(&crc.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn collect_idat(bytes: &[u8]) -> Vec<u8> {
        let mut cursor = 8;
        let mut result = Vec::new();
        while cursor + 12 <= bytes.len() {
            let n = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            let kind = &bytes[cursor + 4..cursor + 8];
            if kind == b"IDAT" { result.extend_from_slice(&bytes[cursor + 8..cursor + 8 + n]); }
            let crc = u32::from_be_bytes(bytes[cursor + 8 + n..cursor + 12 + n].try_into().unwrap());
            assert_eq!(crc, crc32(&bytes[cursor + 4..cursor + 8 + n]));
            cursor += 12 + n;
        }
        result
    }

    fn decode_stored(idat: &[u8]) -> Vec<u8> {
        assert_eq!(&idat[..2], &ZLIB_HEADER);
        let mut out = Vec::new();
        let mut position = 2usize;
        loop {
            let header = idat[position];
            position += 1;
            assert_eq!(header & !1, 0); // BTYPE=00, aligned bits=0
            let n = u16::from_le_bytes([idat[position], idat[position + 1]]) as usize;
            let complement = u16::from_le_bytes([idat[position + 2], idat[position + 3]]);
            assert_eq!(complement, !(n as u16));
            position += 4;
            out.extend_from_slice(&idat[position..position + n]);
            position += n;
            if header & 1 == 1 { break; }
        }
        assert_eq!(position + 4, idat.len());
        let expected = u32::from_be_bytes(idat[position..].try_into().unwrap());
        let mut a=1u32;
        let mut b=0u32;
        for block in out.chunks(5_552) {
            for &v in block { a += u32::from(v); b += a; }
            a %= 65_521; b %= 65_521;
        }
        assert_eq!(expected, b << 16 | a);
        out
    }

    #[test]
    fn png_signature_and_dimensions() {
        let mut bytes = Vec::new();
        encode_bgra(&mut bytes, 2, 1, &[0, 0, 255, 255, 0, 255, 0, 255]).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[16..24], &[0, 0, 0, 2, 0, 0, 0, 1]);
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
        assert_eq!(decode_stored(&collect_idat(&bytes)), vec![0, 255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn multiple_deflate_blocks_and_idat_chunks() {
        let w=640usize;
        let h=150usize;
        let mut src=vec![0u8; w*h*4];
        for (i, px) in src.chunks_exact_mut(4).enumerate() {
            px.copy_from_slice(&[(i & 255) as u8, (i / 3 & 255) as u8, 81, 255]);
        }
        let mut png=Vec::new();
        encode_bgra(&mut png,w as u32,h as u32,&src).unwrap();
        let decoded=decode_stored(&collect_idat(&png));
        assert_eq!(decoded.len(), (w*3+1)*h);
        for y in 0..h {
            assert_eq!(decoded[y*(w*3+1)], 0);
            for x in [0usize,1,100,w-1] {
                let i=(y*w+x)*4;
                let j=y*(w*3+1)+1+x*3;
                assert_eq!(&decoded[j..j+3], &[src[i+2],src[i+1],src[i]]);
            }
        }
    }

    #[test]
    fn reject_bad_image() {
        assert!(encode_bgra(Vec::new(), 5, 5, &[0;4]).is_err());
        assert!(encode_bgra(Vec::new(), u32::MAX, u32::MAX, &[]).is_err());
    }

    #[test]
    fn stream_propagates_errors_without_truncation_success() {
        struct FailImmediately;
        impl Write for FailImmediately {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("full")) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        assert!(encode_bgra(FailImmediately, 1, 1, &[0;4]).is_err());
    }

    #[test]
    fn checksum_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }
}
