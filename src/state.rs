use cosmwasm_std::{Uint128, Decimal,Addr, Deps,StdResult,Order};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw_storage_plus::{Item,Map};

pub const CONFIG: Item<State> = Item::new("config_state");

pub const USERINFO: Map<&str, UserInfo> = Map::new("config_user_info");

pub const USERS : Item<Vec<String>> = Item::new("config_users");


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner:String,
    pub token_address:String,
    pub presale_start:u64,
    pub presale_end:u64,
    pub total_supply:Uint128,
    pub vesting_period:u64,
    pub vesting_step_period:u64,
    pub token_price:Uint128,
    pub token_sold_amount:Uint128,
    pub denom:String,
    pub admin_wallet:String
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UserInfo {
    pub address: String,
    pub total_token:Uint128,
    pub received_token:Uint128,
    pub last_received_time:u64
}
