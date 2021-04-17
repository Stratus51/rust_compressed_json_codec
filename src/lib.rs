use std::collections::HashMap;

pub mod define;
pub mod encoded_data;
pub mod varint;

use encoded_data::EncodedData;

pub struct Conf {}

pub struct Compressor {
    // TODO Object caching
    aliases: Vec<EncodedData>,
    // string_map: HashMap<&str, usize>,
}

impl Compressor {
    pub fn new(conf: Conf) -> Self {
        Self { aliases: vec![] }
    }

    pub fn compress() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let a = 2;
        assert_eq!(2 + a, 4);
    }
}
