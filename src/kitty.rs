//! Kitty graphics protocol: transmitting images and drawing them with
//! Unicode placeholders.
//!
//! An image is transmitted once and given a virtual placement of `cols x rows`
//! cells. The terminal then draws the matching part of the image in every
//! cell that holds U+10EEEE with row and column diacritics, colored with the
//! image id. Those cells are ordinary text to ratatui, so scrolling, margins
//! and redraws move the image with the text.
//!
//! See <https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders>.

use std::fmt::Write as _;

/// The placeholder character; the terminal replaces it with image pixels.
pub(crate) const PLACEHOLDER: char = '\u{10EEEE}';

/// Combining characters that encode a row or column number (and the high
/// byte of the image id): the n-th entry stands for n. Taken from kitty's
/// `gen/rowcolumn-diacritics.txt`.
const DIACRITICS: [char; 297] = [
    '\u{0305}',
    '\u{030D}',
    '\u{030E}',
    '\u{0310}',
    '\u{0312}',
    '\u{033D}',
    '\u{033E}',
    '\u{033F}',
    '\u{0346}',
    '\u{034A}',
    '\u{034B}',
    '\u{034C}',
    '\u{0350}',
    '\u{0351}',
    '\u{0352}',
    '\u{0357}',
    '\u{035B}',
    '\u{0363}',
    '\u{0364}',
    '\u{0365}',
    '\u{0366}',
    '\u{0367}',
    '\u{0368}',
    '\u{0369}',
    '\u{036A}',
    '\u{036B}',
    '\u{036C}',
    '\u{036D}',
    '\u{036E}',
    '\u{036F}',
    '\u{0483}',
    '\u{0484}',
    '\u{0485}',
    '\u{0486}',
    '\u{0487}',
    '\u{0592}',
    '\u{0593}',
    '\u{0594}',
    '\u{0595}',
    '\u{0597}',
    '\u{0598}',
    '\u{0599}',
    '\u{059C}',
    '\u{059D}',
    '\u{059E}',
    '\u{059F}',
    '\u{05A0}',
    '\u{05A1}',
    '\u{05A8}',
    '\u{05A9}',
    '\u{05AB}',
    '\u{05AC}',
    '\u{05AF}',
    '\u{05C4}',
    '\u{0610}',
    '\u{0611}',
    '\u{0612}',
    '\u{0613}',
    '\u{0614}',
    '\u{0615}',
    '\u{0616}',
    '\u{0617}',
    '\u{0657}',
    '\u{0658}',
    '\u{0659}',
    '\u{065A}',
    '\u{065B}',
    '\u{065D}',
    '\u{065E}',
    '\u{06D6}',
    '\u{06D7}',
    '\u{06D8}',
    '\u{06D9}',
    '\u{06DA}',
    '\u{06DB}',
    '\u{06DC}',
    '\u{06DF}',
    '\u{06E0}',
    '\u{06E1}',
    '\u{06E2}',
    '\u{06E4}',
    '\u{06E7}',
    '\u{06E8}',
    '\u{06EB}',
    '\u{06EC}',
    '\u{0730}',
    '\u{0732}',
    '\u{0733}',
    '\u{0735}',
    '\u{0736}',
    '\u{073A}',
    '\u{073D}',
    '\u{073F}',
    '\u{0740}',
    '\u{0741}',
    '\u{0743}',
    '\u{0745}',
    '\u{0747}',
    '\u{0749}',
    '\u{074A}',
    '\u{07EB}',
    '\u{07EC}',
    '\u{07ED}',
    '\u{07EE}',
    '\u{07EF}',
    '\u{07F0}',
    '\u{07F1}',
    '\u{07F3}',
    '\u{0816}',
    '\u{0817}',
    '\u{0818}',
    '\u{0819}',
    '\u{081B}',
    '\u{081C}',
    '\u{081D}',
    '\u{081E}',
    '\u{081F}',
    '\u{0820}',
    '\u{0821}',
    '\u{0822}',
    '\u{0823}',
    '\u{0825}',
    '\u{0826}',
    '\u{0827}',
    '\u{0829}',
    '\u{082A}',
    '\u{082B}',
    '\u{082C}',
    '\u{082D}',
    '\u{0951}',
    '\u{0953}',
    '\u{0954}',
    '\u{0F82}',
    '\u{0F83}',
    '\u{0F86}',
    '\u{0F87}',
    '\u{135D}',
    '\u{135E}',
    '\u{135F}',
    '\u{17DD}',
    '\u{193A}',
    '\u{1A17}',
    '\u{1A75}',
    '\u{1A76}',
    '\u{1A77}',
    '\u{1A78}',
    '\u{1A79}',
    '\u{1A7A}',
    '\u{1A7B}',
    '\u{1A7C}',
    '\u{1B6B}',
    '\u{1B6D}',
    '\u{1B6E}',
    '\u{1B6F}',
    '\u{1B70}',
    '\u{1B71}',
    '\u{1B72}',
    '\u{1B73}',
    '\u{1CD0}',
    '\u{1CD1}',
    '\u{1CD2}',
    '\u{1CDA}',
    '\u{1CDB}',
    '\u{1CE0}',
    '\u{1DC0}',
    '\u{1DC1}',
    '\u{1DC3}',
    '\u{1DC4}',
    '\u{1DC5}',
    '\u{1DC6}',
    '\u{1DC7}',
    '\u{1DC8}',
    '\u{1DC9}',
    '\u{1DCB}',
    '\u{1DCC}',
    '\u{1DD1}',
    '\u{1DD2}',
    '\u{1DD3}',
    '\u{1DD4}',
    '\u{1DD5}',
    '\u{1DD6}',
    '\u{1DD7}',
    '\u{1DD8}',
    '\u{1DD9}',
    '\u{1DDA}',
    '\u{1DDB}',
    '\u{1DDC}',
    '\u{1DDD}',
    '\u{1DDE}',
    '\u{1DDF}',
    '\u{1DE0}',
    '\u{1DE1}',
    '\u{1DE2}',
    '\u{1DE3}',
    '\u{1DE4}',
    '\u{1DE5}',
    '\u{1DE6}',
    '\u{1DFE}',
    '\u{20D0}',
    '\u{20D1}',
    '\u{20D4}',
    '\u{20D5}',
    '\u{20D6}',
    '\u{20D7}',
    '\u{20DB}',
    '\u{20DC}',
    '\u{20E1}',
    '\u{20E7}',
    '\u{20E9}',
    '\u{20F0}',
    '\u{2CEF}',
    '\u{2CF0}',
    '\u{2CF1}',
    '\u{2DE0}',
    '\u{2DE1}',
    '\u{2DE2}',
    '\u{2DE3}',
    '\u{2DE4}',
    '\u{2DE5}',
    '\u{2DE6}',
    '\u{2DE7}',
    '\u{2DE8}',
    '\u{2DE9}',
    '\u{2DEA}',
    '\u{2DEB}',
    '\u{2DEC}',
    '\u{2DED}',
    '\u{2DEE}',
    '\u{2DEF}',
    '\u{2DF0}',
    '\u{2DF1}',
    '\u{2DF2}',
    '\u{2DF3}',
    '\u{2DF4}',
    '\u{2DF5}',
    '\u{2DF6}',
    '\u{2DF7}',
    '\u{2DF8}',
    '\u{2DF9}',
    '\u{2DFA}',
    '\u{2DFB}',
    '\u{2DFC}',
    '\u{2DFD}',
    '\u{2DFE}',
    '\u{2DFF}',
    '\u{A66F}',
    '\u{A67C}',
    '\u{A67D}',
    '\u{A6F0}',
    '\u{A6F1}',
    '\u{A8E0}',
    '\u{A8E1}',
    '\u{A8E2}',
    '\u{A8E3}',
    '\u{A8E4}',
    '\u{A8E5}',
    '\u{A8E6}',
    '\u{A8E7}',
    '\u{A8E8}',
    '\u{A8E9}',
    '\u{A8EA}',
    '\u{A8EB}',
    '\u{A8EC}',
    '\u{A8ED}',
    '\u{A8EE}',
    '\u{A8EF}',
    '\u{A8F0}',
    '\u{A8F1}',
    '\u{AAB0}',
    '\u{AAB2}',
    '\u{AAB3}',
    '\u{AAB7}',
    '\u{AAB8}',
    '\u{AABE}',
    '\u{AABF}',
    '\u{AAC1}',
    '\u{FE20}',
    '\u{FE21}',
    '\u{FE22}',
    '\u{FE23}',
    '\u{FE24}',
    '\u{FE25}',
    '\u{FE26}',
    '\u{10A0F}',
    '\u{10A38}',
    '\u{1D185}',
    '\u{1D186}',
    '\u{1D187}',
    '\u{1D188}',
    '\u{1D189}',
    '\u{1D1AA}',
    '\u{1D1AB}',
    '\u{1D1AC}',
    '\u{1D1AD}',
    '\u{1D242}',
    '\u{1D243}',
    '\u{1D244}',
];

/// Largest number of rows or columns a placement can address.
pub(crate) const MAX_CELLS: u16 = 297;

/// Base64 payload bytes per escape sequence, as the protocol requires.
const CHUNK: usize = 4096;

/// Cell size assumed when the terminal did not report one.
const FALLBACK_CELL_PX: (u16, u16) = (10, 20);

/// The foreground color that carries the low 24 bits of `id`.
pub(crate) fn id_color(id: u32) -> (u8, u8, u8) {
    let [_, r, g, b] = id.to_be_bytes();
    (r, g, b)
}

/// One row of placeholder cells for image `id`: `cols` cells, each with its
/// row and column diacritics, plus the id's high byte when it is not zero.
/// Callers draw it with the foreground color from [`id_color`].
pub(crate) fn placeholder_row(id: u32, row: u16, cols: u16) -> String {
    let high = id.to_be_bytes()[0];
    let row_mark = DIACRITICS[usize::from(row.min(MAX_CELLS - 1))];
    let mut out = String::new();
    for col in 0..cols.min(MAX_CELLS) {
        out.push(PLACEHOLDER);
        out.push(row_mark);
        out.push(DIACRITICS[usize::from(col)]);
        if high != 0 {
            out.push(DIACRITICS[usize::from(high)]);
        }
    }
    out
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = (n >> (18 - 6 * i)) & 0x3f;
                out.push(char::from(BASE64[usize::try_from(index).unwrap_or(0)]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Escape sequences that transmit PNG data as image `id`, in chunks of at
/// most 4096 base64 bytes. Replies are suppressed (`q=2`) so nothing comes
/// back on the pager's input.
pub(crate) fn transmit(id: u32, png: &[u8]) -> Vec<u8> {
    let payload = base64(png);
    let mut out = String::with_capacity(payload.len() + payload.len() / CHUNK * 16 + 64);
    let chunks: Vec<&str> = payload
        .as_bytes()
        .chunks(CHUNK)
        .map(|c| std::str::from_utf8(c).expect("base64 output is ASCII"))
        .collect();
    if chunks.is_empty() {
        let _ = write!(out, "\x1b_Ga=t,f=100,t=d,i={id},q=2;\x1b\\");
    }
    for (n, chunk) in chunks.iter().enumerate() {
        let more = u8::from(n + 1 < chunks.len());
        if n == 0 {
            let _ = write!(out, "\x1b_Ga=t,f=100,t=d,i={id},q=2,m={more};{chunk}\x1b\\");
        } else {
            let _ = write!(out, "\x1b_Gq=2,m={more};{chunk}\x1b\\");
        }
    }
    out.into_bytes()
}

/// Create or resize the virtual placement of image `id` to `cols x rows`
/// cells. Placement 1 is replaced each time, so a resize needs no new data.
pub(crate) fn place(id: u32, cols: u16, rows: u16) -> Vec<u8> {
    format!("\x1b_Ga=p,U=1,i={id},p=1,c={cols},r={rows},q=2\x1b\\").into_bytes()
}

/// Delete image `id` and free its data in the terminal.
pub(crate) fn delete(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\").into_bytes()
}

/// Cells to reserve for an image of `image_px` pixels: its natural size,
/// scaled down (never up) to fit `max_cols x max_rows`, keeping the aspect
/// ratio. `None` when the image or the space is empty.
pub(crate) fn fit_cells(
    image_px: (u32, u32),
    cell_px: (u16, u16),
    max_cols: u16,
    max_rows: u16,
) -> Option<(u16, u16)> {
    let (w, h) = (u64::from(image_px.0), u64::from(image_px.1));
    let max_cols = max_cols.min(MAX_CELLS);
    let max_rows = max_rows.min(MAX_CELLS);
    if w == 0 || h == 0 || max_cols == 0 || max_rows == 0 {
        return None;
    }
    let (cw, ch) = if cell_px.0 == 0 || cell_px.1 == 0 {
        FALLBACK_CELL_PX
    } else {
        cell_px
    };
    let (cw, ch) = (u64::from(cw), u64::from(ch));
    let (max_w, max_h) = (u64::from(max_cols) * cw, u64::from(max_rows) * ch);
    let (disp_w, disp_h) = if w <= max_w && h <= max_h {
        (w, h)
    } else if w * max_h > h * max_w {
        (max_w, (h * max_w / w).max(1))
    } else {
        ((w * max_h / h).max(1), max_h)
    };
    let cells = |px: u64, cell: u64, max: u16| {
        u16::try_from(px.div_ceil(cell)).map_or(max, |n| n.clamp(1, max))
    };
    Some((cells(disp_w, cw, max_cols), cells(disp_h, ch, max_rows)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diacritics_start_and_end_like_kitty() {
        assert_eq!(DIACRITICS[0], '\u{0305}');
        assert_eq!(DIACRITICS[1], '\u{030D}');
        assert_eq!(DIACRITICS[296], '\u{1D244}');
        let unique: std::collections::HashSet<_> = DIACRITICS.iter().collect();
        assert_eq!(unique.len(), DIACRITICS.len());
    }

    #[test]
    fn placeholder_row_encodes_row_and_columns() {
        let row = placeholder_row(42, 2, 3);
        let chars: Vec<char> = row.chars().collect();
        assert_eq!(
            chars,
            vec![
                PLACEHOLDER,
                DIACRITICS[2],
                DIACRITICS[0],
                PLACEHOLDER,
                DIACRITICS[2],
                DIACRITICS[1],
                PLACEHOLDER,
                DIACRITICS[2],
                DIACRITICS[2],
            ]
        );
        // One cell per column: each placeholder is one column wide.
        assert_eq!(unicode_width::UnicodeWidthStr::width(row.as_str()), 3);
    }

    #[test]
    fn placeholder_row_adds_high_id_byte() {
        let id = 0x0500_0001;
        let row = placeholder_row(id, 0, 2);
        let chars: Vec<char> = row.chars().collect();
        assert_eq!(chars.len(), 8);
        assert_eq!(chars[3], DIACRITICS[5]);
        assert_eq!(chars[7], DIACRITICS[5]);
        assert_eq!(id_color(id), (0, 0, 1));
    }

    #[test]
    fn id_color_carries_low_24_bits() {
        assert_eq!(id_color(0x00ab_cdef), (0xab, 0xcd, 0xef));
        assert_eq!(id_color(1), (0, 0, 1));
    }

    #[test]
    fn placeholder_row_caps_at_addressable_cells() {
        let row = placeholder_row(1, 400, 400);
        assert_eq!(row.chars().filter(|&c| c == PLACEHOLDER).count(), 297);
        assert!(row.chars().nth(1) == Some(DIACRITICS[296]));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn transmit_chunks_at_4096_bytes() {
        let data = vec![0u8; 3 * 4096 / 4 * 2 + 10]; // just over two chunks
        let out = String::from_utf8(transmit(7, &data)).unwrap();
        let parts: Vec<&str> = out.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[0].starts_with("\x1b_Ga=t,f=100,t=d,i=7,q=2,m=1;"));
        assert!(parts[1].starts_with("\x1b_Gq=2,m=1;"));
        assert!(parts[2].starts_with("\x1b_Gq=2,m=0;"));
        for part in &parts {
            let payload = part.split_once(';').unwrap().1;
            assert!(payload.len() <= 4096);
        }
        let joined: String = parts.iter().map(|p| p.split_once(';').unwrap().1).collect();
        assert_eq!(joined, base64(&data));
    }

    #[test]
    fn transmit_small_image_is_one_sequence() {
        let out = String::from_utf8(transmit(9, b"png")).unwrap();
        assert_eq!(out, "\x1b_Ga=t,f=100,t=d,i=9,q=2,m=0;cG5n\x1b\\");
    }

    #[test]
    fn place_and_delete_sequences() {
        assert_eq!(
            place(5, 40, 12),
            b"\x1b_Ga=p,U=1,i=5,p=1,c=40,r=12,q=2\x1b\\".to_vec()
        );
        assert_eq!(delete(5), b"\x1b_Ga=d,d=I,i=5,q=2\x1b\\".to_vec());
    }

    #[test]
    fn fit_keeps_small_images_at_natural_size() {
        // 100x40 px with 10x20 cells: 10 columns, 2 rows.
        assert_eq!(fit_cells((100, 40), (10, 20), 80, 30), Some((10, 2)));
        // Partial cells round up so nothing is cut off.
        assert_eq!(fit_cells((101, 41), (10, 20), 80, 30), Some((11, 3)));
    }

    #[test]
    fn fit_scales_wide_images_to_the_column_width() {
        // 2000x1000 px into 80 columns of 10 px = 800 px wide → 400 px high.
        assert_eq!(fit_cells((2000, 1000), (10, 20), 80, 30), Some((80, 20)));
    }

    #[test]
    fn fit_caps_tall_images_at_max_rows() {
        // 500x2000 px: 10 rows of 20 px = 200 px high → 50 px wide.
        assert_eq!(fit_cells((500, 2000), (10, 20), 80, 10), Some((5, 10)));
    }

    #[test]
    fn fit_never_returns_zero_cells() {
        assert_eq!(fit_cells((1, 1), (10, 20), 80, 30), Some((1, 1)));
        assert_eq!(fit_cells((10_000, 1), (10, 20), 5, 5), Some((5, 1)));
        assert_eq!(fit_cells((1, 10_000), (10, 20), 5, 5), Some((1, 5)));
    }

    #[test]
    fn fit_rejects_empty_input_and_space() {
        assert_eq!(fit_cells((0, 10), (10, 20), 80, 30), None);
        assert_eq!(fit_cells((10, 10), (10, 20), 0, 30), None);
        assert_eq!(fit_cells((10, 10), (10, 20), 80, 0), None);
    }

    #[test]
    fn fit_uses_a_fallback_cell_size() {
        assert_eq!(fit_cells((100, 40), (0, 0), 80, 30), Some((10, 2)));
    }

    #[test]
    fn fit_clamps_to_addressable_cells() {
        let (cols, rows) = fit_cells((100_000, 100_000), (1, 1), 1000, 1000).unwrap();
        assert!(cols <= MAX_CELLS && rows <= MAX_CELLS);
    }
}
