pub const SPECIAL: u8 = 0;
pub const INTEGER: u8 = 1;
pub const FLOAT: u8 = 2;
pub const STRING: u8 = 3;
pub const ARRAY: u8 = 4;
pub const OBJECT: u8 = 5;
pub const ALIAS: u8 = 6;

#[repr(u8)]
pub enum DataType {
    Special = SPECIAL,
    // TODO first size bit is sign
    // TODO size: 0, sign: 0 => boolean
    Integer = INTEGER,
    Float = FLOAT,
    String = STRING,
    // TODO Should the first size bit of a container size indicate if the values' types are all the
    // same, allowing single type specification for the container?
    Array = ARRAY,
    Object = OBJECT,
    Alias = ALIAS,
}

impl DataType {
    pub fn from(n: u8) -> Option<Self> {
        Some(match n {
            SPECIAL => Self::Special,
            INTEGER => Self::Integer,
            FLOAT => Self::Float,
            STRING => Self::String,
            ARRAY => Self::Array,
            OBJECT => Self::Object,
            _ => return None,
        })
    }
}
