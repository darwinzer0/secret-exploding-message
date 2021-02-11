use cosmwasm_std::{to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier, Storage, Uint128, HumanAddr, StdResult, StdError};
use std::string::String;
use std::convert::TryFrom;

use crate::msg::{HandleMsg, InitMsg, QueryMsg, ResponseStatus, HandleAnswer, PingResponse};
use crate::state::{save, Config, CONFIG_KEY, load, Message, SEQ_KEY, MessageQueueStorage, MessageStorage};
use crate::msg::ResponseStatus::{Success, Failure};
use secret_toolkit::utils::{pad_handle_result};

/// pad handle responses and log attributes to blocks of 256 bytes to prevent leaking info based on
/// response size
pub const BLOCK_SIZE: usize = 256;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let max_messages = match valid_max_messages(msg.max_messages) {
        Some(v) => v,
        None => return Err(StdError::generic_err("Invalid max_messages."))
    };
    let seq_start = match valid_seq_start(msg.seq_start) {
        Some(v) => v,
        None => return Err(StdError::generic_err("Invalid seq_start."))
    };
    let max_message_size = match valid_max_message_size(msg.max_message_size) {
        Some(v) => v,
        None => return Err(StdError::generic_err("Invalid max_message_size."))
    };

    let config = Config {
        max_messages,
        discard: msg.discard,
        max_message_size,
    };

    save(&mut deps.storage, CONFIG_KEY, &config)?;
    save(&mut deps.storage, SEQ_KEY, &seq_start)?;

    Ok(InitResponse::default())
}

fn valid_max_messages(val: i32) -> Option<u32> {
    if val < 1 {
        None
    } else {
        u32::try_from(val).ok()
    }
}

fn valid_seq_start(val: Uint128) -> Option<u128> {
    let v = val.u128();
    if v < 1 {
        None
    } else {
        Some(v)
    }
}

// we limit the max message size to 65535
fn valid_max_message_size(val: i32) -> Option<u16> {
    if val < 1 {
        None
    } else {
        u16::try_from(val).ok()
    }
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let response = match msg {
        HandleMsg::Send { content, target } => try_send(deps, env, content, target),
        HandleMsg::Recv { } => try_receive(deps, env),
        HandleMsg::Size { } => try_size(deps, env),
        HandleMsg::Block { address } => try_block(deps, env, address),
        HandleMsg::Unblock { address } => try_unblock(deps, env, address),
    };
    pad_handle_result(response, BLOCK_SIZE)
}

pub fn try_send<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    content: String,
    target: HumanAddr,
) -> StdResult<HandleResponse> {
    let status: ResponseStatus;
    let mut response_message = String::new();

    let config: Config = load(&mut deps.storage, CONFIG_KEY)?;
    let seq: u128 = load(&mut deps.storage, SEQ_KEY)?;

    let content_byte_slice: &[u8] = content.as_bytes();
    if content_byte_slice.len() > config.max_message_size.into() {
        status = Failure;
        response_message.push_str(&format!("Message is too long."));
    } else {
        let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
        let target_address_raw = deps.api.canonical_address(&target)?;

        let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
        let mut message_queue = message_queue_storage.get_message_queue(&target_address_raw);

        if message_queue.blocked.contains(&sender_address_raw.as_slice().to_vec()) {
            status = Failure;
            response_message.push_str(&format!("Message could not be sent."));
        } else if (message_queue.length == config.max_messages) && config.discard {
            status = Failure;
            response_message.push_str(&format!("Message could not be sent."));
        } else {
            let mut message_storage = MessageStorage::from_storage(&mut deps.storage);

            // will only happen if config.discard is false
            if message_queue.length == config.max_messages {
                // remove front message
                let front_message: Option<Message> = message_storage.get_message(&message_queue.front);
                if let Some(found_front_message) = front_message {
                    // remove the front message
                    message_storage.remove_message(&message_queue.front);
                    message_queue.front = found_front_message.next;
                    message_queue.length -= 1;
                } else {
                    // this should never happen (empty queue but also length equal to max)
                    return Err(StdError::generic_err("Corrupted message queue."));
                }
            }
            // prepare new message get rear message
            let mut new_message = Message {
                content: content_byte_slice.to_vec(),
                from: sender_address_raw,
                prev: 0,
                next: 0
            };

            // get current rear message
            let rear_message: Option<Message> = message_storage.get_message(&message_queue.rear);

            if let Some(mut found_rear_message) = rear_message {
                found_rear_message.next = seq.clone();
                // update rear message in the message storage
                message_storage.set_message(&message_queue.rear, found_rear_message);
                new_message.prev = message_queue.rear.clone();
            } else {
                // message is first entry in queue
                message_queue.front = seq.clone();
            }
            message_storage.set_message(&seq.clone(), new_message);
            message_queue.rear = seq.clone();
            message_queue.length += 1;

            // update the message queue in storage
            let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
            message_queue_storage.set_message_queue(&target_address_raw, message_queue);

            // increment message id sequence
            save(&mut deps.storage, SEQ_KEY, &(seq + 1))?;

            status = Success;
            response_message.push_str(&format!("Message sent."));
        }
    }
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Send {
            status,
            message: response_message,
        })?),
    })
}

pub fn try_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let status: ResponseStatus;
    let mut response_message = String::new();
    let mut number_of_unread_messages: u32 = 0;
    let mut content: Option<String> = None;
    let mut sender: Option<HumanAddr> = None;

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
    let mut message_queue = message_queue_storage.get_message_queue(&sender_address_raw);

    if message_queue.length == 0 {
        status = Failure;
        response_message.push_str(&format!("No messages."));
    } else {
        // get message at front of the queue
        let mut message_storage = MessageStorage::from_storage(&mut deps.storage);
        // remove front message
        let mes: Option<Message> = message_storage.get_message(&message_queue.front);
        if let Some(found_mes) = mes {
            content = String::from_utf8(found_mes.content).ok();
            sender = deps.api.human_address(&found_mes.from).ok();
            // explode the message
            message_storage.remove_message(&message_queue.front);
            message_queue.front = found_mes.next;
            message_queue.length -= 1;
            number_of_unread_messages = message_queue.length.clone();

            // store new version of message queue
            let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
            message_queue_storage.set_message_queue(&sender_address_raw, message_queue);
            status = Success;
        } else {
            // this should never happen (queue length > 0 but front message is not in message store)
            return Err(StdError::generic_err("Corrupted message queue."));
        }
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Recv {
            status,
            message: response_message,
            number_of_unread_messages,
            content,
            sender,
        })?),
    })
}

pub fn try_size<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let status: ResponseStatus;
    let config: Config = load(&mut deps.storage, CONFIG_KEY)?;
    let response_message = String::from(&format!("Maximum number of messages allowed: {}", config.max_messages));

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
    let message_queue = message_queue_storage.get_message_queue(&sender_address_raw);
    status = Success;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Size {
            status,
            message: response_message,
            number_of_unread_messages: message_queue.length,
        })?),
    })
}

pub fn try_block<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let blocked_address_raw = deps.api.canonical_address(&address)?;
    let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
    let mut message_queue = message_queue_storage.get_message_queue(&sender_address_raw);
    if !message_queue.blocked.contains(&blocked_address_raw.as_slice().to_vec()) {
        // only write to the storage if needed
        message_queue.blocked.insert(blocked_address_raw.as_slice().to_vec());
        message_queue_storage.set_message_queue(&sender_address_raw, message_queue);
    }

    let status: ResponseStatus = Success;
    let response_message = String::from(&format!("Address {} blocked.", address));

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Block {
            status,
            message: response_message,
        })?),
    })
}

pub fn try_unblock<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let blocked_address_raw = deps.api.canonical_address(&address)?;
    let mut message_queue_storage = MessageQueueStorage::from_storage(&mut deps.storage);
    let mut message_queue = message_queue_storage.get_message_queue(&sender_address_raw);
    if message_queue.blocked.contains(&blocked_address_raw.as_slice().to_vec()) {
        // only write to the storage if needed
        message_queue.blocked.remove(&blocked_address_raw.as_slice().to_vec());
        message_queue_storage.set_message_queue(&sender_address_raw, message_queue);
    }

    let status: ResponseStatus = Success;
    let response_message = String::from(&format!("Address {} unblocked.", address));

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Unblock {
            status,
            message: response_message,
        })?),
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Ping {} => to_binary(&query_ping()?),
    }
}

fn query_ping() -> StdResult<PingResponse> {
    Ok(PingResponse{ response: String::from("pong") })
}

