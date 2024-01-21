use std::{collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Arc}};

use async_trait::async_trait;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use crate::storage::{storage_server::Storage, MessageInput, MessageOutput};




#[derive(Debug)]
pub enum StorageServiceError {
    KeyPatternNotValid(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key(pub String);

// lazy initialization of global data: the regex to validate the key
static KEY_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Z]{1}-[a-z0-9]{5}-[A-Z]{1}$").expect("Failed to compile regex"));

impl Key {
    pub fn new(key: String) -> Result<Self, StorageServiceError> {
        match KEY_PATTERN.is_match(&key) {
            true => Ok(Self(key)),
            false => Err(StorageServiceError::KeyPatternNotValid(format!(
                "Key {} is not valid",
                key
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(id: String) -> Self {
        // TODO: add validation
        Self(id)
    }
}

pub type TenantMap = HashMap<TenantId, RwLock<HashMap<Key, u64>>>;

pub struct StorageService {
    pub  last_id: AtomicU64,
    pub  entries: Arc<RwLock<TenantMap>>,
}

#[async_trait]
pub trait InMemoryStorage {
    async fn get_next_id(&self) -> u64;
    async fn insert(&self, tenant: TenantId, key: Key) -> (bool, u64);
}

#[async_trait]
impl InMemoryStorage for StorageService {
    async fn get_next_id(&self) -> u64 {
        self.last_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn insert(&self, tenant_id: TenantId, key: Key) -> (bool, u64) {
        // Case 1. if the tenant map does not exist create it and add the key
        if !self.entries.read().await.contains_key(&tenant_id) {
            let mut tenant_map = HashMap::new();
            let id = self.get_next_id().await;
            tenant_map.insert(key, id);
            self.entries
                .write()
                .await
                .insert(tenant_id.clone(), RwLock::new(tenant_map));
            return (true, id);
        }

        let maybe_value = self
            .entries
            .read()
            .await
            .get(&tenant_id)
            .unwrap()
            .read()
            .await
            .get(&key)
            .cloned();

        match maybe_value {
            // Case 2. if the key does not exist add it
            None => {
                let id = self.get_next_id().await;
                self.entries
                    .write()
                    .await
                    .get(&tenant_id)
                    .unwrap()
                    .write()
                    .await
                    .insert(key, id);
                (true, id)
            }

            // Case 3. if the key exists return the value
            Some(value) => (false, value),
        }
    }
}

#[tonic::async_trait]
impl Storage for StorageService {
    async fn process(
        &self,
        request: Request<MessageInput>,
    ) -> Result<Response<MessageOutput>, Status> {
        //println!("Got a request: {:?}", request);
        let input = request.into_inner();
        let maybe_key = Key::new(input.key);
        match maybe_key {
            Ok(key) => {
                let (new, id) = self.insert(TenantId::new(input.tenant), key).await;
                let output = MessageOutput {
                    new,
                    id,
                    timestamp: Utc::now().timestamp(),
                };
                Ok(Response::new(output))
            }
            Err(e) => Err(Status::invalid_argument(format!("{:?}", e))),
        }
    }
}

impl Default for StorageService {
    fn default() -> Self {
        Self {
            last_id: AtomicU64::new(1),
            entries: Arc::new(RwLock::new(TenantMap::new())),
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::service::{StorageService, Key, TenantId, InMemoryStorage};

    #[test]
    fn test_key_validator_ok() {
        let key = "K-h53dk-A".into();
        assert!(Key::new(key).is_ok())
    }

    #[test]
    fn test_key_validator_ko() {
        let key = "not-a-key".into();
        assert!(Key::new(key).is_err())
    }

    #[tokio::test]
    async fn test_storage_service_default_sample_data() {
        let under_test = StorageService::default();
        let msg1 = (
            TenantId::new("3bd1c697".into()),
            Key::new("K-h53dk-A".into()).unwrap(),
        );
        let msg2 = (
            TenantId::new("75682017".into()),
            Key::new("K-h53dk-A".into()).unwrap(),
        );
        let msg3 = (
            TenantId::new("3bd1c697".into()),
            Key::new("K-867vc-C".into()).unwrap(),
        );
        let msg4 = (
            TenantId::new("75682017".into()),
            Key::new("K-h53dk-A".into()).unwrap(),
        );

        let resp1 = under_test.insert(msg1.0, msg1.1).await;
        let resp2 = under_test.insert(msg2.0, msg2.1).await;
        let resp3 = under_test.insert(msg3.0, msg3.1).await;
        let resp4 = under_test.insert(msg4.0, msg4.1).await;

        assert_eq!(resp1, (true, 1));
        assert_eq!(resp2, (true, 2));
        assert_eq!(resp3, (true, 3));
        assert_eq!(resp4, (false, 2));
    }
}
