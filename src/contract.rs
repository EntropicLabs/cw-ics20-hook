use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Order, Reply,
    Response, StdError, StdResult, SubMsg,
};
use cw_storage_plus::Bound;
use kujira::{IcaMsg, IcaSudoMsg, KujiraMsg};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    CallbackType, ExecuteMsg, InstantiateMsg, QueryMsg, TransferResult, TransferStartedResponse,
    TransferStatus,
};
use crate::state::{next_transfer_id, NEXT_TRANSFER_ID, PENDING_TRANSFERS};

// version info for migration info
const CONTRACT_NAME: &str = "entropic/cw-ics20-hook";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cw_serde]
struct TransferCallback {
    id: u128,
    sender: Addr,
    amount: Coin,
    callback: CallbackType,
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
    deps: DepsMut,
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
            callback,
        } => {
            if info.funds.is_empty() {
                return Err(ContractError::NoFunds {});
            }
            let next_key = next_transfer_id(deps.storage)?;
            PENDING_TRANSFERS.save(deps.storage, next_key, &info.funds)?;
            NEXT_TRANSFER_ID.save(deps.storage, &next_key)?;

            let transfer_msgs = info
                .funds
                .into_iter()
                .map(|coin| {
                    let callback = TransferCallback {
                        id: next_key,
                        sender: info.sender.clone(),
                        amount: coin.clone(),
                        callback: transfer_callback.clone(),
                    };
                    StdResult::Ok(KujiraMsg::Ica(IcaMsg::Transfer {
                        channel_id: channel_id.clone(),
                        to_address: to_address.clone(),
                        amount: coin.clone(),
                        timeout: timeout.clone(),
                        callback: to_json_binary(&callback)?,
                    }))
                })
                .collect::<StdResult<Vec<_>>>()?;

            let callback_msgs = if let Some(callback) = callback {
                vec![callback.to_message(
                    &info.sender,
                    TransferStartedResponse {
                        id: next_key.into(),
                    },
                    vec![],
                )?]
            } else {
                vec![]
            };

            Response::new()
                .add_messages(transfer_msgs)
                .add_messages(callback_msgs)
        }
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TransferStatus { id } => {
            let next_id = next_transfer_id(deps.storage)?;
            let pending = PENDING_TRANSFERS.may_load(deps.storage, id.u128())?;
            if pending.is_none() {
                if id.u128() < next_id {
                    Ok(to_json_binary(&TransferStatus {
                        id,
                        pending: vec![],
                    })?)
                } else {
                    Err(StdError::not_found(format!(
                        "Transfer with id {id} not found",
                    )))
                }
            } else {
                Ok(to_json_binary(&TransferStatus {
                    id,
                    pending: pending.unwrap(),
                })?)
            }
        }
        QueryMsg::AllTransfers { limit, start_after } => {
            let start = start_after.map(Bound::exclusive);
            let transfers = PENDING_TRANSFERS
                .range(deps.storage, start, None, Order::Ascending)
                .take(limit.unwrap_or(10) as usize)
                .map(|item| {
                    item.map(|(id, pending)| TransferStatus {
                        id: id.into(),
                        pending,
                    })
                })
                .collect::<StdResult<Vec<_>>>()?;
            Ok(to_json_binary(&transfers)?)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: IcaSudoMsg) -> Result<Response, ContractError> {
    Ok(match msg {
        IcaSudoMsg::TransferReceipt(_) => Response::default(),
        IcaSudoMsg::TransferCallback(transfer) => {
            let callback: TransferCallback = from_json(&transfer.callback)?;
            let mut pending_transfers = PENDING_TRANSFERS.load(deps.storage, callback.id)?;
            pending_transfers.retain(|coin| coin != &callback.amount);
            let result = match transfer.result {
                kujira::IcaTxResult::Success { .. } => TransferResult::Success,
                kujira::IcaTxResult::Error { error } => TransferResult::Error(error),
                kujira::IcaTxResult::Timeout {} => TransferResult::Timeout,
            };
            let msgs = match callback.callback {
                CallbackType::AfterAny(cb) => {
                    vec![cb.to_message(&callback.sender, &result, vec![])?]
                }
                CallbackType::AfterAll(cb) => {
                    if pending_transfers.is_empty() {
                        vec![cb.to_message(&callback.sender, &result, vec![])?]
                    } else {
                        vec![]
                    }
                }
            };

            if pending_transfers.is_empty() {
                PENDING_TRANSFERS.remove(deps.storage, callback.id);
            } else {
                PENDING_TRANSFERS.save(deps.storage, callback.id, &pending_transfers)?;
            }

            Response::new().add_submessages(msgs.into_iter().map(|m| SubMsg::reply_on_error(m, 0)))
        }
        _ => unreachable!(),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, _msg: Reply) -> Result<Response, ContractError> {
    Ok(Response::default())
}
#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{
        message_info, mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage,
    };
    use cosmwasm_std::{
        coin, coins, to_json_binary, Addr, Binary, CosmosMsg, IbcTimeout, OwnedDeps, Timestamp,
        WasmMsg,
    };
    use kujira::{CallbackData, CallbackMsg, IcaTxResult, KujiraMsg, TransferCallbackData};

    #[cw_serde]
    /// Serialization Helper for Callbacks
    enum ReceiverExecuteMsg {
        Callback(CallbackMsg),
    }

    fn setup_contract() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator = deps.api.addr_make("creator");
        let info = message_info(&creator, &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
        deps
    }

    #[test]
    fn proper_instantiation() {
        let _ = setup_contract();
    }

    #[test]
    fn execute_transfer_success() {
        let mut deps = setup_contract();
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let msg = ExecuteMsg::Transfer {
            channel_id: "channel-0".to_string(),
            to_address: "recipient".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000000000)),
            transfer_callback: CallbackType::AfterAny(callback_data),
            callback: None,
        };
        let sender = deps.api.addr_make("sender");
        let info = message_info(&sender, &coins(100, "uusd"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());

        if let CosmosMsg::Custom(KujiraMsg::Ica(IcaMsg::Transfer {
            channel_id,
            to_address,
            amount,
            timeout,
            callback,
        })) = &res.messages[0].msg
        {
            assert_eq!(channel_id, "channel-0");
            assert_eq!(to_address, "recipient");
            assert_eq!(amount, &Coin::new(100u128, "uusd"));
            assert_eq!(
                timeout,
                &IbcTimeout::with_timestamp(Timestamp::from_seconds(1000000000))
            );
            // Verify the callback data
            let expected_callback = to_json_binary(&TransferCallback {
                id: 1, // Assuming this is the first transfer
                sender,
                amount: Coin::new(100u128, "uusd"),
                callback: CallbackType::AfterAny(CallbackData(Binary::from(b"callback_msg"))),
            })
            .unwrap();
            assert_eq!(callback, &expected_callback);
        } else {
            panic!("Unexpected message type");
        }

        // Check if the transfer is properly stored in PENDING_TRANSFERS
        let pending = PENDING_TRANSFERS.load(deps.as_ref().storage, 1).unwrap();
        assert_eq!(pending, vec![Coin::new(100u128, "uusd")]);
    }

    #[test]
    fn execute_transfer_no_funds() {
        let mut deps = setup_contract();
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let msg = ExecuteMsg::Transfer {
            channel_id: "channel-0".to_string(),
            to_address: "recipient".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000000000)),
            transfer_callback: CallbackType::AfterAny(callback_data),
            callback: None,
        };
        let sender = deps.api.addr_make("sender");
        let info = message_info(&sender, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(matches!(res, Err(ContractError::NoFunds {})));
    }

    fn setup_transfer(
        deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
        data: &CallbackType,
    ) {
        let msg = ExecuteMsg::Transfer {
            channel_id: "channel-0".to_string(),
            to_address: "recipient".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000000000)),
            transfer_callback: data.clone(),
            callback: None,
        };
        let sender = deps.api.addr_make("sender");
        let info = message_info(&sender, &coins(100, "uusd"));
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    fn sudo_transfer_callback_success() {
        let mut deps = setup_contract();
        // First, execute a transfer to set up the state
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let callback = CallbackType::AfterAny(callback_data.clone());
        setup_transfer(&mut deps, &callback);

        // Now, test the sudo callback
        let callback = TransferCallback {
            id: 1,
            sender: Addr::unchecked("sender"),
            amount: Coin::new(100u128, "uusd"),
            callback: CallbackType::AfterAny(callback_data),
        };
        let sudo_msg = IcaSudoMsg::TransferCallback(TransferCallbackData {
            callback: to_json_binary(&callback).unwrap(),
            result: IcaTxResult::Success {
                data: Binary::default(),
            },
            port: "port".to_string(),
            channel: "channel".to_string(),
            sequence: 1,
            receiver: "recipient".to_string(),
            denom: "uusd".to_string(),
            amount: "100".to_string(),
            memo: "".to_string(),
        });
        let res = sudo(deps.as_mut(), mock_env(), sudo_msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Check that the callback message is correctly constructed
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &res.messages[0].msg
        {
            assert_eq!(contract_addr, "sender");

            let parsed_msg: ReceiverExecuteMsg = from_json(msg).unwrap();
            assert_eq!(
                parsed_msg,
                ReceiverExecuteMsg::Callback(CallbackMsg {
                    data: to_json_binary(&TransferResult::Success).unwrap(),
                    callback: Binary::from(b"callback_msg").into()
                })
            );
        } else {
            panic!("Unexpected message type");
        }

        // Check that the transfer is removed from PENDING_TRANSFERS
        let pending = PENDING_TRANSFERS
            .may_load(deps.as_ref().storage, 1)
            .unwrap();
        assert!(pending.is_none());
    }

    #[test]
    fn sudo_transfer_callback_error() {
        let mut deps = setup_contract();
        // First, execute a transfer to set up the state
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let callback = CallbackType::AfterAny(callback_data.clone());
        setup_transfer(&mut deps, &callback);

        // Now, test the sudo callback
        let callback = TransferCallback {
            id: 1,
            sender: Addr::unchecked("sender"),
            amount: Coin::new(100u128, "uusd"),
            callback: CallbackType::AfterAny(callback_data),
        };
        let sudo_msg = IcaSudoMsg::TransferCallback(TransferCallbackData {
            callback: to_json_binary(&callback).unwrap(),
            result: IcaTxResult::Error {
                error: "Transfer failed".to_string(),
            },
            port: "port".to_string(),
            channel: "channel".to_string(),
            sequence: 1,
            receiver: "recipient".to_string(),
            denom: "uusd".to_string(),
            amount: "100".to_string(),
            memo: "".to_string(),
        });
        let res = sudo(deps.as_mut(), mock_env(), sudo_msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Check that the error callback message is correctly constructed
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &res.messages[0].msg
        {
            assert_eq!(contract_addr, "sender");

            let parsed_msg: ReceiverExecuteMsg = from_json(msg).unwrap();
            assert_eq!(
                parsed_msg,
                ReceiverExecuteMsg::Callback(CallbackMsg {
                    data: to_json_binary(&TransferResult::Error("Transfer failed".to_string()))
                        .unwrap(),
                    callback: Binary::from(b"callback_msg").into()
                })
            );
        } else {
            panic!("Unexpected message type");
        }
    }

    #[test]
    fn sudo_transfer_callback_timeout() {
        let mut deps = setup_contract();
        // First, execute a transfer to set up the state
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let callback = CallbackType::AfterAny(callback_data.clone());
        setup_transfer(&mut deps, &callback);

        // Now, test the sudo callback
        let callback = TransferCallback {
            id: 1,
            sender: Addr::unchecked("sender"),
            amount: Coin::new(100u128, "uusd"),
            callback: CallbackType::AfterAny(callback_data),
        };
        let sudo_msg = IcaSudoMsg::TransferCallback(TransferCallbackData {
            callback: to_json_binary(&callback).unwrap(),
            result: IcaTxResult::Timeout {},
            port: "port".to_string(),
            channel: "channel".to_string(),
            sequence: 1,
            receiver: "recipient".to_string(),
            denom: "uusd".to_string(),
            amount: "100".to_string(),
            memo: "".to_string(),
        });
        let res = sudo(deps.as_mut(), mock_env(), sudo_msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Check that the timeout callback message is correctly constructed
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &res.messages[0].msg
        {
            assert_eq!(contract_addr, "sender");
            let parsed_msg: ReceiverExecuteMsg = from_json(msg).unwrap();
            assert_eq!(
                parsed_msg,
                ReceiverExecuteMsg::Callback(CallbackMsg {
                    data: to_json_binary(&TransferResult::Timeout).unwrap(),
                    callback: Binary::from(b"callback_msg").into()
                })
            );
        } else {
            panic!("Unexpected message type");
        }
    }

    #[test]
    fn sudo_transfer_callback_after_all_complete() {
        let mut deps = setup_contract();
        // First, execute a transfer to set up the state
        let callback_data = CallbackData(Binary::from(b"callback_msg"));
        let callback = CallbackType::AfterAll(callback_data.clone());
        setup_transfer(&mut deps, &callback);

        // Now, test the sudo callback
        let callback = TransferCallback {
            id: 1,
            sender: Addr::unchecked("sender"),
            amount: Coin::new(100u128, "uusd"),
            callback: CallbackType::AfterAll(callback_data),
        };
        let sudo_msg = IcaSudoMsg::TransferCallback(TransferCallbackData {
            callback: to_json_binary(&callback).unwrap(),
            result: IcaTxResult::Success {
                data: Binary::default(),
            },
            port: "port".to_string(),
            channel: "channel".to_string(),
            sequence: 1,
            receiver: "recipient".to_string(),
            denom: "uusd".to_string(),
            amount: "100".to_string(),
            memo: "".to_string(),
        });
        let res = sudo(deps.as_mut(), mock_env(), sudo_msg).unwrap();
        assert_eq!(1, res.messages.len());

        // Check that the AfterAll callback message is correctly constructed
        if let CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr, msg, ..
        }) = &res.messages[0].msg
        {
            assert_eq!(contract_addr, "sender");

            let parsed_msg: ReceiverExecuteMsg = from_json(msg).unwrap();
            assert_eq!(
                parsed_msg,
                ReceiverExecuteMsg::Callback(CallbackMsg {
                    data: to_json_binary(&TransferResult::Success).unwrap(),
                    callback: Binary::from(b"callback_msg").into()
                })
            );
        } else {
            panic!("Unexpected message type");
        }

        // Check that the transfer is removed from PENDING_TRANSFERS
        let pending = PENDING_TRANSFERS
            .may_load(deps.as_ref().storage, 1)
            .unwrap();
        assert!(pending.is_none());
    }

    #[test]
    fn sudo_transfer_callback_after_all_pending() {
        let mut deps = setup_contract();
        // Setup with AfterAll callback and multiple pending transfers
        let callback_data = CallbackData(Binary::from(b"callback_msg"));

        let exec_msg = ExecuteMsg::Transfer {
            channel_id: "channel".to_string(),
            to_address: "recipient".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000000000)),
            transfer_callback: CallbackType::AfterAll(callback_data.clone()),
            callback: None,
        };
        let sender = deps.api.addr_make("sender");
        let info = message_info(&sender, &[coin(100, "uusd"), coin(200, "uusk")]);
        execute(deps.as_mut(), mock_env(), info, exec_msg).unwrap();

        // Execute sudo callback for the first coin of the transfer
        let callback = TransferCallback {
            id: 1,
            sender: Addr::unchecked("sender"),
            amount: Coin::new(100u128, "uusd"),
            callback: CallbackType::AfterAll(callback_data),
        };
        let sudo_msg = IcaSudoMsg::TransferCallback(TransferCallbackData {
            callback: to_json_binary(&callback).unwrap(),
            result: IcaTxResult::Success {
                data: Binary::default(),
            },
            port: "port".to_string(),
            channel: "channel".to_string(),
            sequence: 1,
            receiver: "recipient".to_string(),
            denom: "uusd".to_string(),
            amount: "100".to_string(),
            memo: "".to_string(),
        });
        let res = sudo(deps.as_mut(), mock_env(), sudo_msg).unwrap();

        // Check that no callback message is sent yet
        assert_eq!(0, res.messages.len());

        // Check that the first coin is removed from PENDING_TRANSFERS
        let pending1 = PENDING_TRANSFERS
            .may_load(deps.as_ref().storage, 1)
            .unwrap();
        assert!(pending1.is_some());
        assert_eq!(pending1.unwrap(), coins(200, "uusk"));
    }
}
