# Kujira ICS20 Hook Contract

## Overview

This CosmWasm contract serves as a wrapper around Kujira's ICS20 transfer callbacks. It provides a mechanism to execute callbacks after interchain transfers are completed.

## Features

- Execute interchain transfers with customizable callbacks
- Handle transfer success, errors, and timeouts
- Customizable timeout for transfers

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
        transfer_callback: CallbackData,
    },
}
```

- `channel_id`: The IBC channel ID for the transfer
- `to_address`: The recipient address on the destination chain
- `timeout`: The timeout for the IBC transfer
- `transfer_callback`: CallbackData payload to receive with acknowledgements or errors/timeouts.

Note: The actual coin to be transferred should be sent along with this message.

## Error Handling

In case of a transfer error or timeout, the `TransferResult` will be set to `Error` or `Timeout`, respectively, and return the original transfer amount.

If the sender contract has an error during the callback execution, it will NOT revert the entire transaction, as the IBC acknowledgement will be saved anyway, and the initial funds will be bank transferred to the original sender instead.
