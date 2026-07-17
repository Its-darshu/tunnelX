use crate::TunnelFrame;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("failed to encode frame: {0}")]
    Encode(#[from] bincode::Error),
    #[error("failed to decode frame: {0}")]
    Decode(#[from] bincode::ErrorKind),
}

pub fn encode_frame(frame: &TunnelFrame) -> Result<Vec<u8>, CodecError> {
    Ok(bincode::serialize(frame)?)
}

pub fn decode_frame(bytes: &[u8]) -> Result<TunnelFrame, CodecError> {
    bincode::deserialize(bytes).map_err(|e| CodecError::Decode(*e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TunnelFrame;

    #[test]
    fn encode_decode_heartbeat() {
        let frame = TunnelFrame::Heartbeat;
        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }
}
