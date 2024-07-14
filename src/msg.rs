use cosmwasm_schema::cw_serde;
use cosmwasm_std::IbcTimeout;
use kujira::CallbackData;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
/// The returned result of the ICS20 transfer.
pub enum TransferResult {
    Success,
    Error(String),
    Timeout,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Transfer a sent token across the specified ICS20 channel, to the specified address.
    /// Transfer callbacks will be triggered after the transfer result is received.
    Transfer {
        channel_id: String,
        to_address: String,
        timeout: IbcTimeout,
        transfer_callback: CallbackData,
    },
}
