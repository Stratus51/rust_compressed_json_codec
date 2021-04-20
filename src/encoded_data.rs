use crate::define::{
    data_type::{self, DataType},
    special_type::{self, SpecialType},
};
use crate::varint;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq, Clone)]
pub enum EncodedSpecial {
    None,
    Null,
    Define(Box<EncodedData>),
    Forget(u64),
}

#[derive(Debug, PartialEq, Clone)]
pub enum EncodedInteger {
    Positive(u64),
    Negative(u64),
    Bool(bool),
}

#[derive(Debug, PartialEq, Clone)]
pub enum EncodedData {
    Special(EncodedSpecial),
    Integer(EncodedInteger),
    Float(f64),
    String(String),
    Array(Vec<EncodedData>),
    Object(HashMap<String, EncodedData>),
    Alias(u64),
}

impl From<serde_json::Value> for EncodedData {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Special(EncodedSpecial::Null),
            serde_json::Value::Bool(b) => Self::Integer(EncodedInteger::Bool(b)),
            serde_json::Value::Number(n) => {
                if let Some(n) = n.as_u64() {
                    Self::Integer(EncodedInteger::Positive(n))
                } else if let Some(n) = n.as_i64() {
                    Self::Integer(EncodedInteger::Negative(-n as u64))
                } else {
                    Self::Float(n.as_f64().unwrap())
                }
            }
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(list) => {
                Self::Array(list.into_iter().map(|o| o.into()).collect())
            }
            serde_json::Value::Object(map) => {
                Self::Object(map.into_iter().map(|(k, o)| (k, o.into())).collect())
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum EncodedDataToJsonError {
    NegativeIntegerTooBig(u64),
    BadFloat(f64),
    UnsupportedAliasDataType,
    UnsupportedNoneDataType,
    UnsupportedDefineDataType,
    UnsupportedForgetDataType,
}

impl TryFrom<EncodedData> for serde_json::Value {
    type Error = EncodedDataToJsonError;
    fn try_from(v: EncodedData) -> Result<Self, Self::Error> {
        Ok(match v {
            EncodedData::Special(EncodedSpecial::Null) => Self::Null,
            EncodedData::Special(EncodedSpecial::None) => {
                return Err(EncodedDataToJsonError::UnsupportedNoneDataType)
            }
            EncodedData::Special(EncodedSpecial::Define(_)) => {
                return Err(EncodedDataToJsonError::UnsupportedDefineDataType)
            }
            EncodedData::Special(EncodedSpecial::Forget(_)) => {
                return Err(EncodedDataToJsonError::UnsupportedForgetDataType)
            }
            EncodedData::Integer(EncodedInteger::Bool(b)) => Self::Bool(b),
            EncodedData::Integer(EncodedInteger::Positive(n)) => Self::Number((n).into()),
            EncodedData::Integer(EncodedInteger::Negative(n)) => Self::Number(
                (-i64::try_from(n)
                    .map_err(|_| EncodedDataToJsonError::NegativeIntegerTooBig(n))?)
                .into(),
            ),
            EncodedData::Float(n) => Self::Number(
                serde_json::Number::from_f64(n).ok_or(EncodedDataToJsonError::BadFloat(n))?,
            ),
            EncodedData::String(s) => Self::String(s),
            EncodedData::Array(list) => Self::Array(
                list.into_iter()
                    .map(|o| o.try_into())
                    .collect::<Result<_, _>>()?,
            ),
            EncodedData::Object(map) => Self::Object(
                map.into_iter()
                    .map(|(k, o)| o.try_into().map(|v| (k.clone(), v)))
                    .collect::<Result<_, _>>()?,
            ),
            EncodedData::Alias(_) => return Err(EncodedDataToJsonError::UnsupportedAliasDataType),
        })
    }
}

fn encode_compact_u64(n: u64) -> Vec<u8> {
    // TODO Manage the u24, u40, u48, u56 cases
    if n <= 0xFF {
        (n as u8).to_le_bytes().to_vec()
    } else if n <= 0xFF_FF {
        (n as u16).to_le_bytes().to_vec()
    } else if n <= 0xFF_FF_FF_FF {
        (n as u32).to_le_bytes().to_vec()
    } else {
        (n as u64).to_le_bytes().to_vec()
    }
}

unsafe fn decode_compact_u64(data: &[u8], size: u8) -> u64 {
    // TODO Manage the u24, u40, u48, u56 cases
    if size == 1 {
        *data.get_unchecked(0) as u64
    } else if size == 2 {
        let mut n_data = [0u8; 2];
        n_data.clone_from_slice(data.get_unchecked(0..2));
        u16::from_le_bytes(n_data) as u64
    } else if size == 4 {
        let mut n_data = [0u8; 4];
        n_data.clone_from_slice(data.get_unchecked(0..4));
        u32::from_le_bytes(n_data) as u64
    } else {
        let mut n_data = [0u8; 8];
        n_data.clone_from_slice(data.get_unchecked(0..8));
        u64::from_le_bytes(n_data)
    }
}

fn encode_data_type_length(mut n: u64, max_flag_bit: u8) -> (u8, Vec<u8>) {
    let max_flag_size = 1 << (max_flag_bit - 1);
    let flag_mask = (max_flag_size >> 1) - 1;
    if n < max_flag_size {
        (n as u8, vec![])
    } else {
        let continue_flag = 1 << (max_flag_bit - 1);
        n -= max_flag_size;
        let flag_value = (n & flag_mask as u64) as u8 | continue_flag;
        n >>= max_flag_bit - 1;
        (flag_value, varint::encode(n))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DecodeError {
    UnknownDataType(u8),
    UnknownSpecialType(u8),
    MissingBytes(usize),
    VarintTooBig,
    BadUtf8(std::str::Utf8Error),
}

impl EncodedData {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Special(spe) => match spe {
                EncodedSpecial::None => {
                    vec![data_type::SPECIAL << 5 | special_type::NONE]
                }
                EncodedSpecial::Null => {
                    vec![data_type::SPECIAL << 5 | special_type::NULL]
                }
                EncodedSpecial::Define(o) => [
                    vec![data_type::SPECIAL << 5 | special_type::DEFINE],
                    o.encode(),
                ]
                .concat(),
                EncodedSpecial::Forget(id) => [
                    vec![data_type::SPECIAL << 5 | special_type::FORGET],
                    varint::encode(*id),
                ]
                .concat(),
            },
            Self::Integer(int) => match int {
                EncodedInteger::Positive(n) => {
                    let encoded = encode_compact_u64(*n);
                    let data_size = encoded.len() as u8;
                    [vec![data_type::INTEGER << 5 | data_size], encoded].concat()
                }
                EncodedInteger::Negative(n) => {
                    let encoded = encode_compact_u64(*n);
                    let data_size = encoded.len() as u8;
                    [vec![data_type::INTEGER << 5 | 1 << 4 | data_size], encoded].concat()
                }
                EncodedInteger::Bool(b) => {
                    let b_flag = if *b { 1 } else { 0 };
                    vec![data_type::INTEGER << 5 | b_flag << 4]
                }
            },
            Self::Float(f) => {
                [vec![data_type::FLOAT << 5 | 8].as_slice(), &f.to_le_bytes()].concat()
            }
            Self::String(s) => {
                let (flag, data_type_length_data) = encode_data_type_length(s.len() as u64, 5);
                [
                    vec![data_type::STRING << 5 | flag].as_slice(),
                    &data_type_length_data,
                    s.as_bytes(),
                ]
                .concat()
            }
            Self::Array(array) => {
                let (flag, data_type_length_data) = encode_data_type_length(array.len() as u64, 5);
                let mut ret = vec![vec![data_type::ARRAY << 5 | flag], data_type_length_data];
                for o in array.iter() {
                    ret.push(o.encode());
                }
                ret.concat()
            }
            Self::Object(map) => {
                let (flag, data_type_length_data) = encode_data_type_length(map.len() as u64, 5);
                let mut ret = vec![vec![data_type::OBJECT << 5 | flag], data_type_length_data];
                for (k, o) in map.iter() {
                    ret.push(varint::encode(k.len() as u64));
                    ret.push(k.as_bytes().to_vec());
                    ret.push(o.encode());
                }
                ret.concat()
            }
            Self::Alias(id) => {
                let (flag, id_data) = encode_data_type_length(*id, 5);
                [vec![data_type::ALIAS << 5 | flag], id_data].concat()
            }
        }
    }

    pub fn decode(data: &[u8]) -> Result<(Self, usize), DecodeError> {
        unsafe {
            if data.is_empty() {
                return Err(DecodeError::MissingBytes(1));
            }
            let ctrl = data.get_unchecked(0);
            let data_type_value = ctrl >> 5;
            let data_type = match DataType::from(data_type_value) {
                Some(data_type) => data_type,
                None => return Err(DecodeError::UnknownDataType(data_type_value)),
            };
            Ok(match data_type {
                DataType::Special => {
                    let special_type_value = ctrl & 0x1F;
                    let special_type = match SpecialType::from(special_type_value) {
                        Some(special_type) => special_type,
                        None => return Err(DecodeError::UnknownSpecialType(special_type_value)),
                    };
                    match special_type {
                        SpecialType::None => (Self::Special(EncodedSpecial::None), 1),
                        SpecialType::Null => (Self::Special(EncodedSpecial::Null), 1),
                        SpecialType::Define => {
                            if data.len() < 2 {
                                return Err(DecodeError::MissingBytes(1));
                            }
                            let (object, size) = Self::decode(&data.get_unchecked(1..))?;
                            (
                                Self::Special(EncodedSpecial::Define(Box::new(object))),
                                1 + size,
                            )
                        }
                        SpecialType::Forget => {
                            let (id, size) = match varint::decode(data.get_unchecked(1..)) {
                                Ok(e) => e,
                                Err(varint::DecodeError::MissingBytes) => {
                                    return Err(DecodeError::MissingBytes(1))
                                }
                                Err(varint::DecodeError::ValueTooBig) => {
                                    return Err(DecodeError::VarintTooBig)
                                }
                            };
                            (Self::Special(EncodedSpecial::Forget(id)), 1 + size as usize)
                        }
                    }
                }
                DataType::Integer => {
                    let length = ctrl & 0x0F;
                    let negative = ctrl & 0x10 != 0;
                    if length == 0 {
                        (Self::Integer(EncodedInteger::Bool(negative)), 1)
                    } else {
                        if data.len() < 1 + length as usize {
                            return Err(DecodeError::MissingBytes(
                                1 + length as usize - data.len(),
                            ));
                        }
                        let n = decode_compact_u64(data.get_unchecked(1..), length);
                        if negative {
                            (
                                Self::Integer(EncodedInteger::Negative(n)),
                                1 + length as usize,
                            )
                        } else {
                            (
                                Self::Integer(EncodedInteger::Positive(n)),
                                1 + length as usize,
                            )
                        }
                    }
                }
                DataType::Float => {
                    let mut f_data = [0u8; 8];
                    f_data.clone_from_slice(&data.get_unchecked(1..));
                    (Self::Float(f64::from_le_bytes(f_data)), 1 + 8)
                }
                DataType::String => {
                    let length = ctrl & 0x0F;
                    let length_continue = ctrl & 0x10 != 0;
                    let (length, size) = if length_continue {
                        let (length_head, size) = match varint::decode(data.get_unchecked(1..)) {
                            Ok(e) => e,
                            Err(varint::DecodeError::MissingBytes) => {
                                return Err(DecodeError::MissingBytes(1))
                            }
                            Err(varint::DecodeError::ValueTooBig) => {
                                return Err(DecodeError::VarintTooBig)
                            }
                        };
                        (
                            ((length_head as usize) << 4 | length as usize) + 0x10,
                            1 + size as usize,
                        )
                    } else {
                        (length as usize, 1)
                    };
                    let payload = data.get_unchecked(size..size + length);
                    let s = match std::str::from_utf8(payload) {
                        Ok(s) => s.to_string(),
                        Err(e) => return Err(DecodeError::BadUtf8(e)),
                    };
                    (Self::String(s), size + length)
                }
                DataType::Array => {
                    let length = ctrl & 0x0F;
                    let length_continue = ctrl & 0x10 != 0;
                    let (length, size) = if length_continue {
                        let (length_head, size) = match varint::decode(data.get_unchecked(1..)) {
                            Ok(e) => e,
                            Err(varint::DecodeError::MissingBytes) => {
                                return Err(DecodeError::MissingBytes(1))
                            }
                            Err(varint::DecodeError::ValueTooBig) => {
                                return Err(DecodeError::VarintTooBig)
                            }
                        };
                        (
                            ((length_head as usize) << 4 | length as usize) + 0x10,
                            1 + size as usize,
                        )
                    } else {
                        (length as usize, 1)
                    };
                    let mut list = vec![];
                    let mut data_ref = data.get_unchecked(size..);
                    let mut tot_size = size;
                    for _ in 0..length {
                        let (o, size) = Self::decode(data_ref)?;
                        list.push(o);
                        data_ref = data_ref.get_unchecked(size..);
                        tot_size += size;
                    }

                    (Self::Array(list), tot_size)
                }
                DataType::Object => {
                    let length = ctrl & 0x0F;
                    let length_continue = ctrl & 0x10 != 0;
                    let (length, size) = if length_continue {
                        let (length_head, size) = match varint::decode(data.get_unchecked(1..)) {
                            Ok(e) => e,
                            Err(varint::DecodeError::MissingBytes) => {
                                return Err(DecodeError::MissingBytes(1))
                            }
                            Err(varint::DecodeError::ValueTooBig) => {
                                return Err(DecodeError::VarintTooBig)
                            }
                        };
                        (
                            ((length_head as usize) << 4 | length as usize) + 0x10,
                            1 + size as usize,
                        )
                    } else {
                        (length as usize, 1)
                    };
                    let mut map = HashMap::new();
                    let mut data_ref = data.get_unchecked(size..);
                    let mut tot_size = size;
                    for _ in 0..length {
                        let (k_length, size) = match varint::decode(data_ref) {
                            Ok(e) => e,
                            Err(varint::DecodeError::MissingBytes) => {
                                return Err(DecodeError::MissingBytes(1))
                            }
                            Err(varint::DecodeError::ValueTooBig) => {
                                return Err(DecodeError::VarintTooBig)
                            }
                        };
                        let (k_length, size) = (k_length as usize, size as usize);
                        tot_size += size;
                        data_ref = data_ref.get_unchecked(size..);
                        if data_ref.len() < k_length {
                            return Err(DecodeError::MissingBytes(k_length - data_ref.len()));
                        }
                        tot_size += k_length;
                        let k = match std::str::from_utf8(data_ref.get_unchecked(..k_length)) {
                            Ok(k) => k,
                            Err(e) => return Err(DecodeError::BadUtf8(e)),
                        };
                        data_ref = data_ref.get_unchecked(k_length..);
                        let (o, size) = Self::decode(data_ref)?;
                        map.insert(k.to_string(), o);
                        data_ref = data_ref.get_unchecked(size..);
                        tot_size += size;
                    }

                    (Self::Object(map), tot_size)
                }
                DataType::Alias => {
                    let id = ctrl & 0x0F;
                    let id_continue = ctrl & 0x10 != 0;
                    let (id, size) = if id_continue {
                        let (id_head, size) = match varint::decode(data.get_unchecked(1..)) {
                            Ok(e) => e,
                            Err(varint::DecodeError::MissingBytes) => {
                                return Err(DecodeError::MissingBytes(1))
                            }
                            Err(varint::DecodeError::ValueTooBig) => {
                                return Err(DecodeError::VarintTooBig)
                            }
                        };
                        ((id_head as u64) << 3 | id as u64, 1 + size as usize)
                    } else {
                        (id as u64, 1)
                    };
                    (Self::Alias(id), size)
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistency() {
        fn check(object: EncodedData, size: usize) {
            let data = object.encode();
            assert_eq!(data.len(), size);
            let (decoded, decoded_size) = EncodedData::decode(&data).unwrap();
            assert_eq!(decoded, object);
            assert_eq!(decoded_size, size);
        }

        fn check_w_json(object: EncodedData, size: usize) {
            check(object.clone(), size);
            let json: serde_json::Value = object.clone().try_into().unwrap();
            let reencoded: EncodedData = json.into();
            assert_eq!(reencoded, object);
        }

        check(EncodedData::Special(EncodedSpecial::None), 1);
        check_w_json(EncodedData::Special(EncodedSpecial::Null), 1);
        check(
            EncodedData::Special(EncodedSpecial::Define(Box::new(EncodedData::Special(
                EncodedSpecial::Null,
            )))),
            2,
        );
        check(EncodedData::Special(EncodedSpecial::Forget(4)), 2);
        check_w_json(EncodedData::Integer(EncodedInteger::Positive(0)), 2);
        check(EncodedData::Integer(EncodedInteger::Negative(0)), 2);
        check_w_json(EncodedData::Integer(EncodedInteger::Positive(1)), 2);
        check_w_json(EncodedData::Integer(EncodedInteger::Positive(0xFF_FF)), 3);
        check_w_json(
            EncodedData::Integer(EncodedInteger::Negative(0xFF_FF_FF_FF)),
            5,
        );
        check_w_json(EncodedData::Integer(EncodedInteger::Bool(true)), 1);
        check_w_json(EncodedData::Integer(EncodedInteger::Bool(false)), 1);
        check_w_json(EncodedData::Float(1.2f64), 9);
        check_w_json(EncodedData::String("abc".to_string()), 4);
        check_w_json(
            EncodedData::String("1234567890ABCDEF".to_string()),
            1 + 1 + 16,
        );
        check_w_json(
            EncodedData::String("1234567890ABCDEF1234567890ABCDEF".to_string()),
            1 + 1 + 32,
        );
        let array = EncodedData::Array(vec![
            EncodedData::Special(EncodedSpecial::Null),
            EncodedData::Integer(EncodedInteger::Positive(5)),
            EncodedData::String("abc".to_string()),
        ]);
        check_w_json(array.clone(), 1 + 1 + 2 + 4);
        let mut map = HashMap::new();
        map.insert(
            "null".to_string(),
            EncodedData::Special(EncodedSpecial::Null),
        );
        map.insert(
            "positive".to_string(),
            EncodedData::Integer(EncodedInteger::Positive(5)),
        );
        map.insert("string".to_string(), EncodedData::String("abc".to_string()));
        check_w_json(EncodedData::Object(map.clone()), 1 + 5 + 1 + 9 + 2 + 7 + 4);

        let mut new_map = HashMap::new();
        new_map.insert(
            "null".to_string(),
            EncodedData::Special(EncodedSpecial::Null),
        );
        new_map.insert(
            "positive".to_string(),
            EncodedData::Integer(EncodedInteger::Positive(5)),
        );
        new_map.insert("map".to_string(), EncodedData::Object(map));
        new_map.insert("array".to_string(), array);
        check_w_json(
            EncodedData::Object(new_map),
            1 + 5 + 1 + 9 + 2 + 4 + 1 + 5 + 1 + 9 + 2 + 7 + 4 + 6 + 1 + 1 + 2 + 4,
        );
    }
}
