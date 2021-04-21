use crate::encoded_data::{self, EncodedData};
use std::collections::HashMap;

pub struct Conf {
    // TODO This cache limit is hard to use as it is very loosely correlated with the real RAM
    // usage.
    pub max_cache: usize,
    pub max_future_cache: usize,
}

pub enum DecodeError {
    BadFormat(encoded_data::DecodeError),
}

pub struct CacheEntry {
    index: usize,
    max_gain: usize,
    nb_use: usize,
}

impl CacheEntry {
    pub fn new(index: usize, max_gain: usize) -> Self {
        Self {
            index,
            nb_use: 0,
            max_gain,
        }
    }
}

pub struct PotentialCacheEntry {
    max_gain: usize,
    nb_use: usize,
}

impl PotentialCacheEntry {
    pub fn new(max_gain: usize) -> Self {
        Self {
            nb_use: 0,
            max_gain,
        }
    }
}

pub struct StringCache {
    cache: HashMap<String, CacheEntry>,
    future_cache: HashMap<String, PotentialCacheEntry>,
}

pub enum CachingResult {
    Some(usize),
    FutureCached,
    None,
}

impl StringCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            future_cache: HashMap::new(),
        }
    }

    pub fn get_best_gains(&self) -> Vec<(&str, usize)> {
        let mut gains = vec![];
        for (k, entry) in self.cache.iter() {
            gains.push((k.as_str(), entry.max_gain * entry.nb_use));
        }
        for (k, entry) in self.future_cache.iter() {
            gains.push((k.as_str(), entry.max_gain * entry.nb_use));
        }
        gains.sort_by_key(|(_, gain)| *gain);
        gains.into_iter().rev().collect()
    }

    pub fn get_cached(&mut self, s: &str, available_future_cache: bool) -> CachingResult {
        if let Some(cache) = self.cache.get_mut(s) {
            cache.nb_use += 1;
            CachingResult::Some(cache.index)
        } else if let Some(cache) = self.future_cache.get_mut(s) {
            cache.nb_use += 1;
            CachingResult::None
        } else if available_future_cache {
            let mut s_length = s.len();
            let mut s_length_size = 1;
            s_length >>= encoded_data::STRING_FLAG_LENGTH_SIZE;
            while s_length > 0 {
                s_length_size += 1;
                s_length <<= 7;
            }
            let gain = s_length_size + s.len();
            let min_loss = 1;
            if min_loss < gain {
                self.future_cache
                    .insert(s.to_string(), PotentialCacheEntry::new(gain - min_loss));
            }
            CachingResult::FutureCached
        } else {
            CachingResult::None
        }
    }
}

pub struct Cache {
    strings: StringCache,
    available_cache: usize,
    available_future_cache: usize,
}

impl Cache {
    pub fn new(max_cache: usize, max_future_cache: usize) -> Self {
        Self {
            strings: StringCache::new(),
            available_cache: max_cache,
            available_future_cache: max_future_cache,
        }
    }

    pub fn get_cached(&mut self, o: &EncodedData) -> Option<usize> {
        match o {
            EncodedData::String(s) => {
                match self.strings.get_cached(s, self.available_future_cache > 0) {
                    CachingResult::Some(index) => Some(index),
                    CachingResult::None => None,
                    CachingResult::FutureCached => {
                        self.available_future_cache -= 1;
                        None
                    }
                }
            }
            _ => None,
        }
    }
}

pub struct StreamCompressor {
    cache: Cache,
}

impl StreamCompressor {
    pub fn new(conf: Conf) -> Self {
        Self {
            cache: Cache::new(conf.max_cache, conf.max_future_cache),
        }
    }

    pub fn compress(object: &EncodedData) -> Vec<u8> {
        object.encode()
    }

    pub fn decompress(data: &[u8]) -> Result<(EncodedData, usize), DecodeError> {
        let (decoded, size) = EncodedData::decode(data).map_err(DecodeError::BadFormat)?;

        Ok((decoded, size))
    }
}
