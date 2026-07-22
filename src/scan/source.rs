use std::fs;
use std::path::Path;

use crate::model::SourceFailureReason;

pub(super) fn read_source(path: &Path) -> Result<String, SourceFailureReason> {
    let bytes = fs::read(path).map_err(|_| SourceFailureReason::IoError)?;
    decode_source(&bytes)
}

fn decode_source(bytes: &[u8]) -> Result<String, SourceFailureReason> {
    if let Some(payload) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(payload.to_vec())
            .map_err(|_| SourceFailureReason::InvalidEncoding);
    }
    if let Some(payload) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return decode_utf16(payload, u16::from_le_bytes);
    }
    if let Some(payload) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return decode_utf16(payload, u16::from_be_bytes);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| SourceFailureReason::UnsupportedEncoding)
}

fn decode_utf16(
    bytes: &[u8],
    decode_unit: fn([u8; 2]) -> u16,
) -> Result<String, SourceFailureReason> {
    if bytes.len() % 2 != 0 {
        return Err(SourceFailureReason::InvalidEncoding);
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| decode_unit([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| SourceFailureReason::InvalidEncoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_supported_unicode_encodings() {
        assert_eq!(decode_source(b"fn plain() {}"), Ok("fn plain() {}".into()));
        assert_eq!(
            decode_source(b"\xef\xbb\xbffn bom() {}"),
            Ok("fn bom() {}".into())
        );
        let text = "fn wide() {}";
        let mut little = vec![0xff, 0xfe];
        little.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let mut big = vec![0xfe, 0xff];
        big.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(decode_source(&little), Ok(text.into()));
        assert_eq!(decode_source(&big), Ok(text.into()));
    }

    #[test]
    fn classifies_unsupported_and_invalid_encodings() {
        assert_eq!(
            decode_source(&[0x80]),
            Err(SourceFailureReason::UnsupportedEncoding)
        );
        assert_eq!(
            decode_source(&[0xff, 0xfe, 0x00]),
            Err(SourceFailureReason::InvalidEncoding)
        );
        assert_eq!(
            decode_source(&[0xef, 0xbb, 0xbf, 0x80]),
            Err(SourceFailureReason::InvalidEncoding)
        );
    }

    #[test]
    fn classifies_file_read_errors() {
        let missing =
            std::env::temp_dir().join(format!("reforge-missing-source-{}", std::process::id()));
        assert_eq!(read_source(&missing), Err(SourceFailureReason::IoError));
    }
}
