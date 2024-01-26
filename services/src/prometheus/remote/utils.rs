use super::types::{PrometheusRemoteStorageError, PrometheusResult};

pub(crate) fn decode_snappy(raw: &[u8]) -> PrometheusResult<Vec<u8>> {
    let mut decoder = snap::raw::Decoder::new();
    decoder
        .decompress_vec(raw)
        .map_err(PrometheusRemoteStorageError::Snappy)
}

pub(crate) fn encode_snappy(data: &[u8]) -> PrometheusResult<Vec<u8>> {
    let mut encoder = snap::raw::Encoder::new();
    encoder
        .compress_vec(data)
        .map_err(PrometheusRemoteStorageError::Snappy)
}
