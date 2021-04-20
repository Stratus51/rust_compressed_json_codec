use crate::encoded_data::{self, EncodedData};

pub struct Conf {}

pub enum DecodeError {
    BadFormat(encoded_data::DecodeError),
}

pub struct StreamCompressor {
    // TODO Object caching
    aliases: Vec<EncodedData>,
    // string_map: HashMap<&str, usize>,
}

impl StreamCompressor {
    pub fn new(conf: Conf) -> Self {
        Self { aliases: vec![] }
    }

    pub fn compress(object: &EncodedData) -> Vec<u8> {
        object.encode()
    }

    pub fn decompress(data: &[u8]) -> Result<(EncodedData, usize), DecodeError> {
        let (decoded, size) = EncodedData::decode(data).map_err(DecodeError::BadFormat)?;

        Ok((decoded, size))
    }
}
