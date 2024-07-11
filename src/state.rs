use cosmwasm_std::{Coin, StdResult, Storage};
use cw_storage_plus::{Item, Map};

pub(crate) const PENDING_TRANSFERS: Map<u128, Vec<Coin>> = Map::new("pending_transfers");
pub(crate) const NEXT_TRANSFER_ID: Item<u128> = Item::new("next_transfer_id");

pub(crate) fn next_transfer_id(storage: &dyn Storage) -> StdResult<u128> {
    let id = NEXT_TRANSFER_ID.may_load(storage)?.unwrap_or_default();
    Ok(id + 1)
}
