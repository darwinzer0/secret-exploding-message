# Secret Exploding Message

Secret contract for the passing of self-destructing messages on [Secret Network](https://scrt.network). Messages sent using this contract can be read once by the recipient and then they are deleted. Because the contract's data is encrypted no one else can view the contents of the message.

A more in depth description of how I wrote this contract can be found here: <https://darwinzero.medium.com/creating-my-first-secret-contract-on-secret-network-scrt-db0d04597051>. 

![exploding message](https://img.gadgethacks.com/img/92/72/63485919495213/0/send-self-destructing-spy-messages-via-google-docs-texts-and-private-links.w1456.jpg "This message will self-destruct!")

## Initializing the contract

The initialization message takes the following format:

```rust
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
```
`seq_start` is the starting id value for the first message. The id is incremented for each additional message that is sent. The `max_messages` field must be `1` or higher. The `max_message_size` is cast to a `u16`, so must be in `1..65535` or will cause an error message.

There are five types of requests defined for the contract:

```rust
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
```

For responses with a data field it is sent as a padded binary Uint8Array. You will need to decode that. For example, in Javascript you would do something like the following (assuming the `response` variable holds the response from the contract): 

```javascript
let utf8decoder = new TextDecoder();
let data = JSON.parse(utf8decoder.decode(response.data));
```

## Sending messages

Messages are sent using the `send` request with two parameters `content` and `target`. The message is added to the rear of the message queue for the target, unless: 1) the queue is full (#messages == `max_messages`) and `discard` was set to `true` in the initialization message, or 2) the sender has been blocked by the recipient (see below).

## Receiving messages

Receiving a message is done via a `recv` request, rather than a query, because we want to have access to the sender's address.

The messages for each user are stored in a linked queue data structure in the storage. A request to receive a message dequeues the message at the front of the queue, deletes it from the storage, and returns the contents in the response. The number of remaining messages in the queue is also returned. 

## Getting count of messages in queue

The `size` request is used to return the count of messages in queue without reading any message.

## Blocking and unblocking senders

Along with the message queue each user has a HashSet that holds the accounts that are blocked from sending messages. The `block` and `unblock` requests will modify the block list accordingly. 

## Disclaimer

I created this contract to help teach myself Rust and how to program secret contracts that run on [Secret Network](https://github.com/enigmampc/SecretNetwork). Although privacy is baked into the network, no guarantees are made for how secret these messages actually are (e.g., due to data leaks, etc.). Results *are* padded using the [secret-toolkit utilities](https://github.com/enigmampc/secret-toolkit/tree/master/packages/utils), but I have not done an exhaustive evaluation of whether or how metadata such as key length, request message length, and message sending/receiving behavior on the network could leak information. And one final important caveat is that the message does not really "explode" entirely from the blockchain given that it still exists on the chain prior to the deletion (albeit in an encrypted form only accessible to the recipient).
