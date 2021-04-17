pub const NONE: u8 = 0;
pub const NULL: u8 = 1;
pub const DEFINE: u8 = 2;
pub const FORGET: u8 = 3;

#[repr(u8)]
pub enum SpecialType {
    None = NONE,
    Null = NULL,
    Define = DEFINE,
    Forget = FORGET,
}

impl SpecialType {
    pub fn from(n: u8) -> Option<Self> {
        Some(match n {
            NONE => Self::None,
            NULL => Self::Null,
            DEFINE => Self::Define,
            FORGET => Self::Forget,
            _ => return None,
        })
    }
}
