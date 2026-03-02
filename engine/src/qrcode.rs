//! # QR Code Generation
//!
//! Converts data strings into QR code matrices for vector rendering in PDF.
//! Uses the `qrcode` crate for encoding and error correction.

use qrcode::types::QrError;
use qrcode::QrCode;

/// A QR code represented as a grid of boolean modules.
#[derive(Debug, Clone)]
pub struct QrMatrix {
    /// true = dark module, false = light module.
    pub modules: Vec<Vec<bool>>,
    /// Number of modules per side (QR codes are always square).
    pub size: usize,
}

/// Generate a QR code matrix from the given data string.
pub fn generate_qr(data: &str) -> Result<QrMatrix, QrError> {
    let code = QrCode::new(data.as_bytes())?;
    let size = code.width();
    let modules = (0..size)
        .map(|y| {
            (0..size)
                .map(|x| code[(x, y)] == qrcode::Color::Dark)
                .collect()
        })
        .collect();
    Ok(QrMatrix { modules, size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_valid() {
        let matrix = generate_qr("https://formepdf.com").unwrap();
        assert!(matrix.size > 0, "QR matrix should have positive size");
        assert_eq!(matrix.modules.len(), matrix.size, "Rows should match size");
        assert_eq!(
            matrix.modules[0].len(),
            matrix.size,
            "Columns should match size"
        );
    }

    #[test]
    fn test_generate_qr_square() {
        let matrix = generate_qr("test data").unwrap();
        for (i, row) in matrix.modules.iter().enumerate() {
            assert_eq!(row.len(), matrix.size, "Row {i} length should equal size");
        }
    }

    #[test]
    fn test_generate_qr_has_dark_modules() {
        let matrix = generate_qr("hello").unwrap();
        let has_dark = matrix.modules.iter().any(|row| row.iter().any(|&m| m));
        assert!(has_dark, "QR code should have at least some dark modules");
    }
}
