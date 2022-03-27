use std::{sync::RwLock, time::{Instant, Duration}};
use std::hash::Hash;
use linked_hash_map::LinkedHashMap;

pub struct Cache<K: Hash + Eq + Clone, V: Clone> {
	cache: RwLock<LinkedHashMap<K, CacheItem<V>>>,
    invalidation_time: Duration,
    max_size: usize
}

struct CacheItem<T> {
    value: T,
    cache_time: Instant 
}

impl<K: Hash + Eq + Clone, V: Clone> Cache<K, V> {
    pub fn new(invalidation_time: Duration, max_size: usize) -> Self {
        Cache { 
            cache: RwLock::new(LinkedHashMap::new()),
            invalidation_time: invalidation_time,
            max_size: max_size
        }
    }

    pub fn erase(&self, key: &K) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(key);
    }

    pub fn put(&self, key: K, value: V) {
        let mut cache = self.cache.write().unwrap();
        if cache.len() >= self.max_size {
			let first_key = cache.iter().next().unwrap().0.clone();
			cache.remove(&first_key);
		}

        cache.insert(key, CacheItem {
            value: value,
            cache_time: Instant::now()
        });
    }

    pub fn try_get(&self, key: &K) -> Option<V> {
        let cache = self.cache.read().unwrap();

        match cache.get(key) {
            Some(item) => {
                if item.cache_time.elapsed() < self.invalidation_time {
                    Some(item.value.clone())
                }   else {
                    None
                }
            },
            None => None
        }
    }
}
