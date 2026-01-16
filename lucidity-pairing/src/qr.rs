use crate::{PairingPayload, PublicKey};
use anyhow::Result;
use qrcode::{QrCode, render::svg};

/// Generate a pairing QR code as SVG
pub fn generate_pairing_qr(payload: &PairingPayload) -> Result<String> {
    let url = format_pairing_url(payload)?;
    
    let code = QrCode::new(url.as_bytes())?;
    let svg = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    
    Ok(svg)
}

/// Format pairing payload as URL for QR code
fn format_pairing_url(payload: &PairingPayload) -> Result<String> {
    let json = payload.to_json()?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
    Ok(format!("lucidity://pair?data={}", encoded))
}

/// Parse pairing URL from QR code
pub fn parse_pairing_url(url: &str) -> Result<PairingPayload> {
    // Expected format: lucidity://pair?data=<base64>
    if !url.starts_with("lucidity://pair?data=") {
        anyhow::bail!("invalid pairing URL scheme");
    }
    
    let data = url.strip_prefix("lucidity://pair?data=")
        .ok_or_else(|| anyhow::anyhow!("missing data parameter"))?;
    
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data)?;
    let json = String::from_utf8(decoded)?;
    
    PairingPayload::from_json(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Keypair;

    #[test]
    fn qr_url_roundtrip() {
        let keypair = Keypair::generate();
        let payload = PairingPayload::new(keypair.public_key());

        let url = format_pairing_url(&payload).unwrap();
        let decoded = parse_pairing_url(&url).unwrap();

        assert_eq!(payload.desktop_public_key, decoded.desktop_public_key);
        assert_eq!(payload.relay_id, decoded.relay_id);
    }

    #[test]
    fn generate_qr_svg() {
        let keypair = Keypair::generate();
        let payload = PairingPayload::new(keypair.public_key());

        let svg = generate_pairing_qr(&payload).unwrap();
        
        // Should be valid SVG
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn invalid_url_scheme() {
        assert!(parse_pairing_url("http://example.com").is_err());
        assert!(parse_pairing_url("lucidity://invalid").is_err());
    }
}
