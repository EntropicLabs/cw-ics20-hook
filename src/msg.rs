use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, IbcTimeout, Uint128};
use kujira::CallbackData;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum CallbackType {
    /// Callback triggered after every successful transfer.
    AfterAny(CallbackData),
    /// Callback triggered only after all transfers are successful.
    AfterAll(CallbackData),
}

#[cw_serde]
/// The returned result of the ICS20 transfer.
pub enum TransferResult {
    Success,
    Error(String),
    Timeout,
}

#[cw_serde]
/// This contract's internal ID for the transfer.
pub struct TransferStartedResponse {
    pub id: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Transfer any sent tokens across the specified ICS20 channel, to the specified address.
    /// Transfer callbacks will be triggered after the transfer result is received, either after
    /// every transfer or after all transfers are successful.
    ///
    /// Specifying `callback` will return this contract's internal ID for the transfer, which can
    /// be used to query the transfer status.
    Transfer {
        channel_id: String,
        to_address: String,
        timeout: IbcTimeout,
        transfer_callback: CallbackType,
        callback: Option<CallbackData>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Query the status of a transfer by its internal ID.
    #[returns(TransferStatus)]
    TransferStatus { id: Uint128 },
    /// Query the status of all transfers.
    #[returns(Vec<TransferStatus>)]
    AllTransfers {
        limit: Option<u32>,
        start_after: Option<Uint128>,
    },
}

#[cw_serde]
pub struct TransferStatus {
    pub id: Uint128,
    pub pending: Vec<Coin>,
}
