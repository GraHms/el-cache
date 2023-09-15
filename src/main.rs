use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::{Duration, Instant};
use warp::Filter;
use std::sync::{Arc, RwLock};
use serde::Deserialize;

struct CacheEntry<V> {
    value: V,
    expiration: Instant,
}

pub struct Cache<K, V> {
    data: HashMap<K, CacheEntry<V>>,
    order: VecDeque<K>,
    max_size: usize,
}

impl<K, V> Cache<K, V>
    where
        K: Eq + Hash + Clone,
{
    pub fn new(max_size: usize) -> Self {
        Cache {
            data: HashMap::new(),
            order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    fn remove_order(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.data.contains_key(key) {
            let is_expired = {
                let entry = self.data.get(key).unwrap();
                entry.expiration <= Instant::now()
            };

            if is_expired {
                self.delete(key);
                return None;
            } else {
                self.remove_order(key);
                self.order.push_back(key.clone());
            }
        }
        self.data.get(key).map(|entry| &entry.value)
    }


    pub fn set(&mut self, key: K, value: V, ttl: Duration) {
        if self.data.len() == self.max_size {
            if let Some(old_key) = self.order.pop_front() {
                self.data.remove(&old_key);
            }
        }
        let entry = CacheEntry {
            value,
            expiration: Instant::now() + ttl,
        };
        self.data.insert(key.clone(), entry);
        self.order.push_back(key);
    }

    pub fn delete(&mut self, key: &K) -> Option<V> {
        self.remove_order(key);
        self.data.remove(key).map(|entry| entry.value)
    }
}




#[derive(Deserialize)]
struct SetRequestBody {
    key: String,
    value: String,
}

#[derive(Deserialize)]
struct DeleteRequestBody {
    key: String,
}



#[tokio::main]
async fn main() {
    let cache = Cache::new(3);
    let shared_cache = Arc::new(RwLock::new(cache));

    let set_cache = Arc::clone(&shared_cache);
    let delete_cache = Arc::clone(&shared_cache);

    let set_route = warp::path("set")
        .and(warp::any().map(move || Arc::clone(&set_cache)))
        .and(warp::post())
        .and(warp::body::json())
        .and_then(set_handler);

    let delete_route = warp::path("delete")
        .and(warp::any().map(move || Arc::clone(&delete_cache)))
        .and(warp::delete())
        .and(warp::body::json())
        .and_then(delete_handler);


    let routes = set_route.or(delete_route);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn set_handler(
    cache: Arc<RwLock<Cache<String, String>>>,
    body: SetRequestBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut cache = cache.write().unwrap();
    cache.set(body.key, body.value, Duration::from_secs(5));
    Ok(warp::reply::json(&"Set successful"))
}

async fn delete_handler(
    cache: Arc<RwLock<Cache<String, String>>>,
    body: DeleteRequestBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut cache = cache.write().unwrap();
    cache.delete(&body.key);
    Ok(warp::reply::json(&"Delete successful"))
}
