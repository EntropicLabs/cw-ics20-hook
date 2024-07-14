use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, StdError, StdResult, SubMsg,
};
use cw_storage_plus::Item;
use cw_utils::one_coin;
use kujira::{CallbackData, IcaMsg, IcaSudoMsg, KujiraMsg};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, TransferResult};

// version info for migration info
const CONTRACT_NAME: &str = "entropic/cw-ics20-hook";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cw_serde]
struct TransferCallback {
    sender: Addr,
    amount: Coin,
    callback: CallbackData,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<KujiraMsg>, ContractError> {
    Ok(match msg {
        ExecuteMsg::Transfer {
            channel_id,
            to_address,
            timeout,
            transfer_callback,
        } => {
            let transfer_coin = one_coin(&info)?;
            let callback = TransferCallback {
                sender: info.sender.clone(),
                amount: transfer_coin.clone(),
                callback: transfer_callback.clone(),
            };
            let transfer_msg = KujiraMsg::Ica(IcaMsg::Transfer {
                channel_id: channel_id.clone(),
                to_address: to_address.clone(),
                amount: transfer_coin.clone(),
                timeout: timeout.clone(),
                callback: to_json_binary(&callback)?,
            });

            Response::new().add_message(transfer_msg)
        }
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: ()) -> StdResult<Binary> {
    Err(StdError::generic_err("Query not supported"))
}

const ERROR_COIN: Item<(String, Coin)> = Item::new("error_coin");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: IcaSudoMsg) -> Result<Response, ContractError> {
    Ok(match msg {
        IcaSudoMsg::TransferReceipt(_) => Response::default(),
        IcaSudoMsg::TransferCallback(transfer) => {
            let callback: TransferCallback = from_json(&transfer.callback)?;
            let msg = match transfer.result {
                kujira::IcaTxResult::Success { .. } => {
                    callback
                        .callback
                        .to_message(&callback.sender, &TransferResult::Success, vec![])
                }
                kujira::IcaTxResult::Error { error } => {
                    ERROR_COIN.save(
                        deps.storage,
                        &(callback.sender.to_string(), callback.amount.clone()),
                    )?;
                    callback.callback.to_message(
                        &callback.sender,
                        TransferResult::Error(error),
                        vec![callback.amount],
                    )
                }
                kujira::IcaTxResult::Timeout {} => {
                    ERROR_COIN.save(
                        deps.storage,
                        &(callback.sender.to_string(), callback.amount.clone()),
                    )?;
                    callback.callback.to_message(
                        &callback.sender,
                        &TransferResult::Timeout,
                        vec![callback.amount],
                    )
                }
            }?;

            Response::new().add_submessage(SubMsg::reply_on_error(msg, 0))
        }
        _ => unreachable!(),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, _msg: Reply) -> Result<Response, ContractError> {
    let (to_address, error_coin) = ERROR_COIN.load(deps.storage)?;

    Ok(Response::default().add_message(BankMsg::Send {
        to_address,
        amount: vec![error_coin],
    }))
}
