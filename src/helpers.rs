use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_json_binary, Addr, CosmosMsg, CustomMsg, IbcTimeout, StdResult, WasmMsg};

use crate::msg::ExecuteMsg;

/// CwTemplateContract is a wrapper around Addr that provides a lot of helpers
/// for working with this.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CwIcs20Contract(pub Addr);

impl CwIcs20Contract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>, C: CustomMsg>(&self, msg: T) -> StdResult<CosmosMsg<C>> {
        let msg = to_json_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

    pub fn transfer<T: Serialize, C: CustomMsg>(
        &self,
        channel_id: String,
        to_address: String,
        timeout: IbcTimeout,
        transfer_callback: &T,
    ) -> StdResult<CosmosMsg<C>> {
        let msg = ExecuteMsg::Transfer {
            channel_id,
            to_address,
            timeout,
            transfer_callback: to_json_binary(transfer_callback)?.into(),
        };
        let msg = to_json_binary(&msg)?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }
}
