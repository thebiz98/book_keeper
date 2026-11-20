use std::fs;
use std::path::Path;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn make_png(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    // Signature
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // bit depth
    ihdr_data.push(6); // RGBA
    ihdr_data.push(0); // compression
    ihdr_data.push(0); // filter
    ihdr_data.push(0); // interlace

    let mut ihdr_chunk = Vec::new();
    ihdr_chunk.extend_from_slice(b"IHDR");
    ihdr_chunk.extend_from_slice(&ihdr_data);
    let ihdr_crc = crc32(&ihdr_chunk);

    png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
    png.extend_from_slice(&ihdr_chunk);
    png.extend_from_slice(&ihdr_crc.to_be_bytes());

    // Raw image data: height rows, each: filter byte (0) + width * 4 bytes RGBA
    let row_len = 1 + (width as usize) * 4;
    let mut row = Vec::with_capacity(row_len);
    row.push(0); // filter None
    for _ in 0..width {
        row.extend_from_slice(&[40, 100, 220, 255]); // blue icon
    }
    let mut raw_data = Vec::with_capacity(row_len * (height as usize));
    for _ in 0..height {
        raw_data.extend_from_slice(&row);
    }

    // zlib format with uncompressed DEFLATE blocks (max 65535 bytes per block)
    let mut zlib_data = Vec::new();
    zlib_data.extend_from_slice(&[0x78, 0x01]); // CMF, FLG
    let chunks = raw_data.chunks(65535);
    let total_chunks = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        let is_last = i == total_chunks - 1;
        let btype = if is_last { 0x01u8 } else { 0x00u8 };
        zlib_data.push(btype);
        let len = chunk.len() as u16;
        zlib_data.extend_from_slice(&len.to_le_bytes());
        zlib_data.extend_from_slice(&(!len).to_le_bytes());
        zlib_data.extend_from_slice(chunk);
    }
    let adler = adler32(&raw_data);
    zlib_data.extend_from_slice(&adler.to_be_bytes());

    // IDAT
    let mut idat_chunk = Vec::new();
    idat_chunk.extend_from_slice(b"IDAT");
    idat_chunk.extend_from_slice(&zlib_data);
    let idat_crc = crc32(&idat_chunk);

    png.extend_from_slice(&(zlib_data.len() as u32).to_be_bytes());
    png.extend_from_slice(&idat_chunk);
    png.extend_from_slice(&idat_crc.to_be_bytes());

    // IEND
    let mut iend_chunk = Vec::new();
    iend_chunk.extend_from_slice(b"IEND");
    let iend_crc = crc32(&iend_chunk);
    png.extend_from_slice(&0u32.to_be_bytes());
    png.extend_from_slice(&iend_chunk);
    png.extend_from_slice(&iend_crc.to_be_bytes());

    png
}

fn make_ico(png32: &[u8]) -> Vec<u8> {
    let mut ico = Vec::new();
    // ICONDIR
    ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    // ICONDIRENTRY
    ico.push(32); // width
    ico.push(32); // height
    ico.push(0);  // colors
    ico.push(0);  // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
    ico.extend_from_slice(&(png32.len() as u32).to_le_bytes()); // bytes in res
    ico.extend_from_slice(&22u32.to_le_bytes()); // offset (6 + 16)
    ico.extend_from_slice(png32);
    ico
}

fn make_icns(png128: &[u8]) -> Vec<u8> {
    let mut icns = Vec::new();
    let total_len = 8 + 8 + png128.len() as u32;
    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&total_len.to_be_bytes());
    icns.extend_from_slice(b"ic07"); // 128x128 png
    let chunk_len = 8 + png128.len() as u32;
    icns.extend_from_slice(&chunk_len.to_be_bytes());
    icns.extend_from_slice(png128);
    icns
}

fn ensure_icons() {
    let icons_dir = Path::new("icons");
    if !icons_dir.exists() {
        let _ = fs::create_dir_all(icons_dir);
    }

    let png32 = make_png(32, 32);
    let png128 = make_png(128, 128);
    let png256 = make_png(256, 256);
    let png512 = make_png(512, 512);

    let _ = fs::write(icons_dir.join("32x32.png"), &png32);
    let _ = fs::write(icons_dir.join("128x128.png"), &png128);
    let _ = fs::write(icons_dir.join("128x128@2x.png"), &png256);
    let _ = fs::write(icons_dir.join("icon.png"), &png512);
    let _ = fs::write(icons_dir.join("icon.ico"), make_ico(&png32));
    let _ = fs::write(icons_dir.join("icon.icns"), make_icns(&png128));
}

fn main() {
    ensure_icons();
    tauri_build::build();
}
