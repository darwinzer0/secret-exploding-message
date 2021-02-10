use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// initial value of the message id serial
    pub seq_start: Uint128,
    /// maximum number of messages per receiver address
    pub max_messages: i32,
    /// maximum size of a message in bytes
    pub max_message_size: i32,
    /// if discard true, will not push messages to a full queue,
    /// else will dequeue oldest message to make room
    pub discard: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Send {
        content: String,
        target: HumanAddr,
    },
    Recv { },
    Size { },
    Block {
        address: HumanAddr,
    },
    Unblock {
        address: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // ping
    Ping {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PingResponse {
    pub response: String,
}

/// success or failure response
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub enum ResponseStatus {
    Success,
    Failure,
}

/// Responses from handle functions
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    /// response from send attempt
    Send {
        /// success or failure
        status: ResponseStatus,
        /// execution description
        message: String,
    },
    /// response from receive attempt
    Recv {
        /// success or failure
        status: ResponseStatus,
        /// execution description
        message: String,
        /// number of unread messages
        number_of_unread_messages: Option<u32>,
        /// content of message
        content: Option<String>,
        /// sender of message
        sender: Option<HumanAddr>,
    },
    /// response from size of message box attempt
    Size {
        /// success or failure
        status: ResponseStatus,
        /// execution description
        message: String,
        /// number of unread messages
        number_of_unread_messages: Option<u16>,
    },
    /// generic status response
    Status {
        /// success or failure
        status: ResponseStatus,
        /// execution description
        message: String,
    },
}
