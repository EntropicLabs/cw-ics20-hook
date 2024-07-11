# Kujira ICS20 Hook Contract

## Overview

This CosmWasm contract serves as a wrapper around Kujira's ICS20 transfer callbacks. It provides a mechanism to execute callbacks after interchain transfers are completed, either individually or after all transfers in a batch are finished. The contract can handle multiple coin transfers in a single transaction.

## Features

- Execute interchain transfers with customizable callbacks
- Support for multiple coin transfers in a single transaction
- Two types of callbacks:
  - `AfterAny`: Triggered after each successful transfer
  - `AfterAll`: Triggered only after all transfers in a batch are successful
- Handle transfer success, errors, and timeouts
- Customizable timeout for transfers
- Query transfer status by ID or retrieve all transfers

## Contract Messages

### InstantiateMsg

Used to initialize the contract. Currently, it doesn't require any parameters.

```rust
pub struct InstantiateMsg {}
```

### ExecuteMsg

The main message for executing actions on the contract.

```rust
pub enum ExecuteMsg {
    Transfer {
        channel_id: String,
        to_address: String,
        timeout: IbcTimeout,
        transfer_callback: CallbackType,
        callback: Option<CallbackData>,
    },
}
```

- `channel_id`: The IBC channel ID for the transfer
- `to_address`: The recipient address on the destination chain
- `timeout`: The timeout for the IBC transfer
- `transfer_callback`: The type of callback to execute (AfterAny or AfterAll) after the transfer(s)
- `callback`: Immediate callback to the sender after the transfer is initiated, containing the internal ID associated with the transfer

Note: The actual coins to be transferred should be sent along with this message.

### QueryMsg

Queries available in the contract:

```rust
pub enum QueryMsg {
    TransferStatus { id: Uint128 },
    AllTransfers {
        limit: Option<u32>,
        start_after: Option<Uint128>,
    },
}
```

- `TransferStatus`: Query the status of a specific transfer by its ID
- `AllTransfers`: Query the status of all transfers, with optional pagination

## Callback Types

```rust
pub enum CallbackType {
    AfterAny(CallbackData),
    AfterAll(CallbackData),
}
```

- `AfterAny`: Executes the callback after each successful transfer
- `AfterAll`: Executes the callback only after all transfers in a batch are successful

```rust
pub struct TransferStartedResponse {
    pub id: Uint128,
}
```

This is returned in the `data` field in the initial callback after a transfer is initiated.

- `id`: The internal ID associated with the transfer

## Usage

1. Instantiate the contract on your Cosmos-based blockchain.

2. To initiate a transfer with a callback:

   ```rust
   let msg = ExecuteMsg::Transfer {
       channel_id: "channel-0".to_string(),
       to_address: "recipient_address".to_string(),
       timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(future_timestamp)),
       transfer_callback: CallbackType::AfterAny(CallbackData(Binary::from(b"callback_data"))),
       callback: Some(CallbackData(Binary::from(b"initial_callback_data"))),
   };
   ```

   Send this message along with the coins you want to transfer. The contract will initiate an IBC transfer for each coin sent.

3. The contract will handle the transfer(s) and execute the callback based on the specified type and the result of the transfer(s).

4. To query the status of a transfer:

   ```rust
   let query_msg = QueryMsg::TransferStatus { id: Uint128::new(1) };
   ```

5. To query all transfers:

   ```rust
   let query_msg = QueryMsg::AllTransfers { limit: Some(10), start_after: None };
   ```

## Transfer Process

1. When an `ExecuteMsg::Transfer` is received, the contract processes each coin sent with the message.
2. For each coin, an IBC transfer is initiated using the provided channel ID, recipient address, and timeout.
3. The contract keeps track of each transfer's status internally.
4. Callbacks are executed based on the specified type (AfterAny or AfterAll) and the success of the transfer(s).

## Error Handling

In case of a transfer error or timeout, the `TransferResult` will be set to `Error` or `Timeout`, respectively. The contract will still execute the callback(s) based on the specified type.

If the sender contract has an error during the callback execution, it will NOT revert the entire transaction, as the IBC acknowledgement will be saved anyway.
