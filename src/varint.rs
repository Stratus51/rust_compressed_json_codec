const LIMITS: [u64; 9] = [
    0x80,
    0x40_00,
    0x20_00_00,
    0x10_00_00_00,
    0x08_00_00_00_00,
    0x04_00_00_00_00_00,
    0x02_00_00_00_00_00_00,
    0x01_00_00_00_00_00_00_00,
    0x00_80_00_00_00_00_00_00_00,
];
const BASE_VALUE: [u64; 10] = [
    0,
    LIMITS[0],
    LIMITS[0] + LIMITS[1],
    LIMITS[0] + LIMITS[1] + LIMITS[2],
    LIMITS[0] + LIMITS[1] + LIMITS[2] + LIMITS[3],
    LIMITS[0] + LIMITS[1] + LIMITS[2] + LIMITS[3] + LIMITS[4],
    LIMITS[0] + LIMITS[1] + LIMITS[2] + LIMITS[3] + LIMITS[4] + LIMITS[5],
    LIMITS[0] + LIMITS[1] + LIMITS[2] + LIMITS[3] + LIMITS[4] + LIMITS[5] + LIMITS[6],
    LIMITS[0] + LIMITS[1] + LIMITS[2] + LIMITS[3] + LIMITS[4] + LIMITS[5] + LIMITS[6] + LIMITS[7],
    LIMITS[0]
        + LIMITS[1]
        + LIMITS[2]
        + LIMITS[3]
        + LIMITS[4]
        + LIMITS[5]
        + LIMITS[6]
        + LIMITS[7]
        + LIMITS[8],
];

fn std_encode(mut n: u64, nb_bytes: usize) -> Vec<u8> {
    let mut ret = vec![];
    for _ in 0..nb_bytes {
        ret.push(((n & 0x7F) as u8) | 0x80);
        n >>= 7;
    }
    ret[nb_bytes - 1] &= 0x7F;
    ret
}

pub fn encode(mut n: u64) -> Vec<u8> {
    for (nb_bytes, limit) in LIMITS.iter().enumerate() {
        if n < *limit {
            return std_encode(n, nb_bytes + 1);
        }
        n -= limit;
    }
    std_encode(n, LIMITS.len() + 1)
}

#[derive(Debug)]
pub enum DecodeError {
    MissingBytes,
    ValueTooBig,
}

fn std_decode(data: &[u8]) -> Result<(u64, u8), DecodeError> {
    let mut ret = 0u64;
    for (i, part) in data.iter().enumerate().take(LIMITS.len() + 1) {
        ret |= ((part & 0x7F) as u64) << (7 * i);

        if part & 0x80 == 0x00 {
            return Ok((ret, i as u8 + 1));
        }
    }
    if data.len() > LIMITS.len() + 1 {
        Err(DecodeError::ValueTooBig)
    } else {
        Err(DecodeError::MissingBytes)
    }
}

pub fn decode(data: &[u8]) -> Result<(u64, u8), DecodeError> {
    let (extra, nb_bytes) = std_decode(data)?;
    Ok((extra + BASE_VALUE[nb_bytes as usize - 1], nb_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistency() {
        fn check(n: u64, size: u8) {
            let (ret, consumed) = decode(&encode(n)).unwrap();
            assert_eq!(n, ret);
            assert_eq!(size, consumed);
        }

        check(0, 1);
        check(1, 1);
        check(2, 1);
        check(0x7F, 1);
        check(0x80, 2);
        check(0xFF, 2);
        check(0x3F_FF, 2);
        check(0x3F_FF + BASE_VALUE[1], 2);
        check(0x3F_FF + BASE_VALUE[1] + 1, 3);
        check(0x1F_FF_FF, 3);
        check(0x0F_FF_FF_FF, 4);
        check(0x07_FF_FF_FF_FF, 5);
        check(0x03_FF_FF_FF_FF_FF, 6);
        check(0x01_FF_FF_FF_FF_FF_FF, 7);
        check(0x00_FF_FF_FF_FF_FF_FF_FF, 8);
        check(0x00_FF_FF_FF_FF_FF_FF_FF + BASE_VALUE[7], 8);
        check(0x00_FF_FF_FF_FF_FF_FF_FF + BASE_VALUE[7] + 1, 9);
        check(0x00_7F_FF_FF_FF_FF_FF_FF_FF, 9);
        check(0x00_FF_FF_FF_FF_FF_FF_FF_FF, 10);
    }
}
