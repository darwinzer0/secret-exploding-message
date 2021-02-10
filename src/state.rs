use std::{any::type_name,collections::HashSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Storage, ReadonlyStorage, StdResult, StdError};
use serde::de::DeserializeOwned;
use secret_toolkit::serialization::{Bincode2, Serde};
use cosmwasm_storage::{PrefixedStorage};

pub static SEQ_KEY: &[u8] = b"seq";
pub static CONFIG_KEY: &[u8] = b"config";
// keys for messages take form: m{message_id.to_be_bytes()}
pub static MESSAGE_PREFIX: &[u8] = b"mes";
// keys for message box queues take form: q{CanonicalAddr of recipient}
pub static MESSAGE_QUEUE_PREFIX: &[u8] = b"box";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Message {
    pub content: Vec<u8>,
    /// address of the sender
    pub from: CanonicalAddr,
    /// id of prev message, 0 means first in queue
    pub prev: u128,
    /// id of next message in queue, 0 means last in queue
    pub next: u128,
}

pub struct MessageStorage<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> MessageStorage<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(MESSAGE_PREFIX, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyMessageStorageImpl<PrefixedStorage<S>> {
        ReadonlyMessageStorageImpl(&self.storage)
    }

    pub fn set_message(&mut self, key: &u128, mes: Message) {
        save(&mut self.storage, &key.to_be_bytes(), &mes).ok();
    }

    pub fn remove_message(&mut self, key: &u128) {
        remove(&mut self.storage, &key.to_be_bytes());
    }

    pub fn get_message(&mut self, key: &u128) -> Option<Message> {
        self.as_readonly().get(key)
    }
}

struct ReadonlyMessageStorageImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyMessageStorageImpl<'a, S> {
    pub fn get(&self, key: &u128) -> Option<Message> {
        let mes: Option<Message> = may_load(self.0, &key.to_be_bytes()).ok().unwrap();
        mes
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MessageQueue {
    /// id of front message
    pub front: u128,
    /// id of end message
    pub rear: u128,
    /// length of queue
    pub length: u32,
    /// set of blocked addresses (canonical)
    pub blocked: HashSet<Vec<u8>>,
}

pub struct MessageQueueStorage<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> MessageQueueStorage<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(MESSAGE_QUEUE_PREFIX, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyMessageQueueStorageImpl<PrefixedStorage<S>> {
        ReadonlyMessageQueueStorageImpl(&self.storage)
    }

    pub fn set_message_queue(&mut self, key: &CanonicalAddr, queue: MessageQueue) {
        save(&mut self.storage, &key.as_slice().to_vec(), &queue).ok();
    }

    pub fn get_message_queue(&mut self, key: &CanonicalAddr) -> MessageQueue {
        self.as_readonly().get(key)
    }
}

struct ReadonlyMessageQueueStorageImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyMessageQueueStorageImpl<'a, S> {
    pub fn get(&self, key: &CanonicalAddr) -> MessageQueue {
        let queue: Option<MessageQueue> = may_load(self.0, &key.as_slice().to_vec()).ok().unwrap();
        if let Some(found_queue) = queue {
            found_queue
        } else {
            MessageQueue {
                front: 0,
                rear: 0,
                length: 0,
                blocked: HashSet::new()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// maximum number of messages
    pub max_messages: u32,
    /// if discard true, will not push messages to full queue,
    /// else will dequeue oldest message to make room
    pub discard: bool,
    pub max_message_size: u16,
}

/// Returns StdResult<()> resulting from saving an item to storage
///
/// # Arguments
///
/// * `storage` - a mutable reference to the storage this item should go to
/// * `key` - a byte slice representing the key to access the stored item
/// * `value` - a reference to the item to store
// save(&mut deps.storage, CONFIG_KEY, &state)?;
pub fn save<T: Serialize, S: Storage>(storage: &mut S, key: &[u8], value: &T) -> StdResult<()> {
    storage.set(key, &Bincode2::serialize(value)?);
    Ok(())
}

/// Removes an item from storage
///
/// # Arguments
///
/// * `storage` - a mutable reference to the storage this item is in
/// * `key` - a byte slice representing the key that accesses the stored item
pub fn remove<S: Storage>(storage: &mut S, key: &[u8]) {
    storage.remove(key);
}

/// Returns StdResult<T> from retrieving the item with the specified key.  Returns a
/// StdError::NotFound if there is no item with that key
///
/// # Arguments
///
/// * `storage` - a reference to the storage this item is in
/// * `key` - a byte slice representing the key that accesses the stored item
pub fn load<T: DeserializeOwned, S: ReadonlyStorage>(storage: &S, key: &[u8]) -> StdResult<T> {
    Bincode2::deserialize(
        &storage
            .get(key)
            .ok_or_else(|| StdError::not_found(type_name::<T>()))?,
    )
}

/// Returns StdResult<Option<T>> from retrieving the item with the specified key.
/// Returns Ok(None) if there is no item with that key
///
/// # Arguments
///
/// * `storage` - a reference to the storage this item is in
/// * `key` - a byte slice representing the key that accesses the stored item
// let bid: Option<Bid> = may_load(&deps.storage, bidder_raw.as_slice())?;
pub fn may_load<T: DeserializeOwned, S: ReadonlyStorage>(
    storage: &S,
    key: &[u8],
) -> StdResult<Option<T>> {
    match storage.get(key) {
        Some(value) => Bincode2::deserialize(&value).map(Some),
        None => Ok(None),
    }
}


