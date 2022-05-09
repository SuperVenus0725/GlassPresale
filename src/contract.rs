use cosmwasm_std::{
    entry_point, to_binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,Binary,
    StdResult, Uint128,CosmosMsg,WasmMsg,BankMsg
};

use cw2::set_contract_version;
use cw20::{ Cw20ExecuteMsg};


use crate::error::{ContractError};
use crate::msg::{ ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State,CONFIG,UserInfo, USERINFO, USERS};


const CONTRACT_NAME: &str = "Hope_Market_Place";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    if msg.presale_start>msg.presale_end {
        return Err(ContractError::WrongSetTime {  })
    }
    let state = State {
        owner:info.sender.to_string(),
        token_address:String::from("token_address"),
        presale_start:msg.presale_start,
        presale_end:msg.presale_end,
        total_supply:msg.total_supply,
        vesting_period:msg.vesting_period,
        vesting_step_period:msg.vesting_step_period,
        token_price:msg.token_price,
        token_sold_amount:Uint128::new(0),
        denom:msg.denom,
        admin_wallet:msg.admin_wallet,
        can_send:true
    };
    CONFIG.save(deps.storage,&state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
    ExecuteMsg::SendTokenContract {}=>execute_send_token_contract(deps,env,info),
    ExecuteMsg::BuyToken { amount } =>execute_buy_token(deps,env,info,amount),
    ExecuteMsg::WithdrawToken { } => execute_withdraw_token(deps, env, info),
    ExecuteMsg::WithdrawAdminToken { } => execute_withdraw_admin_token(deps, env, info),
    ExecuteMsg::SetTokenAddress {address} => execute_token_address(deps,env,info,address),
    ExecuteMsg::ChangeOwner { address } =>execute_change_owner(deps,env,info,address),
    }
}

//Mint token to this contract
fn execute_send_token_contract(
    deps: DepsMut,
    env:Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let  state = CONFIG.load(deps.storage)?;

    if !state.can_send {
        return Err(ContractError::AlreadySent {})
    }
    
    if info.sender.to_string() != state.owner{
        return Err(ContractError::Unauthorized { })
    }   

    CONFIG.update(deps.storage, 
        |mut state| ->StdResult<_>{
            state.can_send = false;
            Ok(state)
        })?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { 
                recipient: env.contract.address.to_string(), 
                amount: state.total_supply })?,
            funds: vec![],
        })
    ))
}

//buy token with stable coin
fn execute_buy_token(
    deps: DepsMut,
    env:Env,
    info: MessageInfo,
    amount:Uint128
) -> Result<Response, ContractError> {
    let mut state = CONFIG.load(deps.storage)?;

    let current_time = env.block.time.seconds();

    if current_time<state.presale_start{
        return Err(ContractError::PresaleNotStarted {});
    }
    
    if state.token_sold_amount + amount > state.total_supply{
        return Err(ContractError::InsufficientRemainingToken { })
    }   

    let deposit_amount= info
        .funds
        .iter()
        .find(|c| c.denom == state.denom)
        .map(|c| Uint128::from(c.amount))
        .unwrap_or_else(Uint128::zero);
    
    if deposit_amount != state.token_price*amount/Uint128::new(1000000){
        return Err(ContractError::NotExactFunds {})
    }

    let user_info = USERINFO.may_load(deps.storage,&info.sender.to_string())?;
    
    if user_info == None{
        let new_user = UserInfo{
            address:info.sender.to_string(),
            total_token:amount,
            received_token:Uint128::new(0),
            last_received_time :state.presale_end-state.vesting_step_period
        };
        USERINFO.save(deps.storage, &info.sender.to_string(), &new_user)?;
        
        let  users = USERS.may_load(deps.storage)?;
        if users == None{
            let all_users = vec![info.sender.to_string()];
            USERS.save(deps.storage,&all_users)?;    
        }
        else{
            let mut all_users = users.unwrap();
            all_users.push(info.sender.to_string());
             USERS.save(deps.storage,&all_users)?;    
        }    
    }
    else {
        let mut user = user_info.unwrap();
        user.total_token = user.total_token + amount; 
        USERINFO.save(deps.storage, &info.sender.to_string(), &user)?;
    }

    state.token_sold_amount = state.token_sold_amount + amount;
    CONFIG.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_message(CosmosMsg::Bank(
            BankMsg::Send { 
                to_address: state.admin_wallet, 
                amount: vec![Coin{
                    denom:state.denom,
                    amount:deposit_amount
                } ]
            })
    ))
}

fn execute_withdraw_token(
    deps: DepsMut,
    env:Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    
    let state = CONFIG.load(deps.storage)?;
    
    let current_time = env.block.time.seconds();

    if current_time<state.presale_end{
        return Err(ContractError::PresaleNotFinished {  });
    }
    
    let user_info =  USERINFO.may_load(deps.storage,&info.sender.to_string())?;
    
    if user_info == None{
        return Err(ContractError::NotDeposited {  });
    }
    
    let  user_info = user_info.unwrap();

    if env.block.time.seconds()  < state.vesting_step_period + user_info.last_received_time
    {
        return Err(ContractError::NotRemainingToken {  });
    }   
    
    let mut step = state.vesting_period/state.vesting_step_period;
    if step !=0{
        step+=1;
    }
    let current_step = (current_time-user_info.last_received_time)/state.vesting_step_period;

    // //token amout you will receive this time   
    let remaining_token = user_info.total_token/Uint128::from(step as u128)*Uint128::from(current_step as u128);
    
    USERINFO.update(deps.storage, &info.sender.to_string(), 
    |user_info|->StdResult<_>{
        let mut user = user_info.unwrap();
        user.last_received_time = state.presale_end+(current_step-1)*state.vesting_step_period;
        user.received_token += remaining_token;
        Ok(user)
    })?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer { 
                recipient: info.sender.to_string(), 
                amount: remaining_token })?,
            funds: vec![],
        })
    ))
}



fn execute_withdraw_admin_token(
    deps: DepsMut,
    env:Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    
    let state = CONFIG.load(deps.storage)?;
   
    if info.sender.to_string()!=state.admin_wallet{
        return  Err(ContractError::Unauthorized {  });
    }

    let current_time = env.block.time.seconds();
    
    if current_time<state.presale_end{
        return Err(ContractError::PresaleNotFinished {  });
    }

    let remaining_token = state.total_supply - state.token_sold_amount;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer { 
                recipient: info.sender.to_string(), 
                amount: remaining_token })?,
            funds: vec![],
        })
    ))
}



fn execute_token_address(
    deps: DepsMut,
    _env:Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut state = CONFIG.load(deps.storage)?;
    deps.api.addr_validate(&address)?;
    
    state.token_address = address;

    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }

    CONFIG.save(deps.storage, &state)?;
    Ok(Response::default())
}

fn execute_change_owner(
    deps: DepsMut,
    _env:Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut state = CONFIG.load(deps.storage)?;

    if state.owner != info.sender.to_string() {
        return Err(ContractError::Unauthorized {});
    }
    deps.api.addr_validate(&address)?;
    state.owner = address;
    CONFIG.save(deps.storage,&state)?;
    Ok(Response::default())
}



#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStateInfo {} => to_binary(&query_state_info(deps)?),
        QueryMsg::CheckWithdraw {address} => to_binary(&query_check_user(deps,_env,address)?),
        QueryMsg::GetUserInfo { address } => to_binary(&query_user_info(deps,address)?),
        QueryMsg::GetAllUsers{} => to_binary(&query_get_users(deps)?),
    }   
}

pub fn query_state_info(deps:Deps) -> StdResult<State>{
    let state =  CONFIG.load(deps.storage)?;
    Ok(state)
}

pub fn query_user_info(deps:Deps,address:String) -> StdResult<UserInfo>{
    let user_info =  USERINFO.load(deps.storage,&address)?;
    Ok(user_info)
}

pub fn query_get_users(deps: Deps) -> StdResult<Vec<String>> {
    let res = USERS.load(deps.storage)?;
    Ok(res)
}

pub fn query_check_user(deps: Deps,env:Env,address: String) -> StdResult<bool> {
    let user_info =  USERINFO.may_load(deps.storage,&address)?;
    if user_info ==None{
        Ok(false)
    }
    else{
        let user_info = user_info.unwrap();
        let state = CONFIG.load(deps.storage)?;
            if env.block.time.seconds()  > state.vesting_step_period + user_info.last_received_time{
                Ok(true)
            }
            else{
                Ok(false)
            }}
}


#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{ CosmosMsg, Coin};

    #[test]
    fn set_token_address() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds()+120,
            presale_end:env.block.time.seconds()+420,
            total_supply:Uint128::new(1000),
            vesting_period:600,
            vesting_step_period:120,
            token_price:Uint128::new(1),
            denom:"uusd".to_string(),
            admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetTokenAddress  { address:"token_address1".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_address,"token_address1".to_string());

    }

    #[test]
    fn change_owner() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds()+120,
            presale_end:env.block.time.seconds()+420,
            total_supply:Uint128::new(1000),
            vesting_period:600,
            vesting_step_period:120,
            token_price:Uint128::new(1),
            denom:"uusd".to_string(),
             admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());
        assert_eq!(state,State{
            presale_start:env.block.time.seconds()+120,
            presale_end:env.block.time.seconds()+420,
            total_supply:Uint128::new(1000),
            vesting_period:600,
            vesting_step_period:120,
            token_price:Uint128::new(1),
            owner:"creator".to_string(),
            token_address:"token_address".to_string(),
            token_sold_amount:Uint128::new(0),
            denom:"uusd".to_string(),
            admin_wallet:"admin".to_string(),
            can_send:true
        });

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::ChangeOwner { address:"owner".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"owner".to_string());
    }

    #[test]
    fn send_token_contract() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds()+120,
            presale_end:env.block.time.seconds()+420,
            total_supply:Uint128::new(1000),
            vesting_period:600,
            vesting_step_period:120,
            token_price:Uint128::new(1),
            denom:"uusd".to_string(),
            admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetTokenAddress  { address:"token_address1".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_address,"token_address1".to_string());
        //mint token
        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SendTokenContract {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.messages.len(),1);
    }

    #[test]

    fn buy_token() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds(),
            presale_end:env.block.time.seconds()+240,
            total_supply:Uint128::new(10000000),
            vesting_period:500,
            vesting_step_period:125,
            token_price:Uint128::new(10000),
            denom:"uusd".to_string(),
             admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetTokenAddress  { address:"token_address1".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_address,"token_address1".to_string());

        let info = mock_info("buyer1",&[Coin{
            denom:"uusd".to_string(),
            amount:Uint128::new(10000)
        }]);

        let msg = ExecuteMsg::BuyToken { amount: Uint128::new(1000000) };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user_info = query_user_info(deps.as_ref(), "buyer1".to_string() ).unwrap();
        assert_eq!(user_info,UserInfo{
            address:"buyer1".to_string(),
            total_token:Uint128::new(1000000),
            received_token:Uint128::new(0),
            last_received_time:env.block.time.seconds()+115
        });

        let info = mock_info("buyer1",&[Coin{
            denom:"uusd".to_string(),
            amount:Uint128::new(30000)
        }]);
        let msg = ExecuteMsg::BuyToken { amount: Uint128::new(3000000) };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user_info = query_user_info(deps.as_ref(), "buyer1".to_string() ).unwrap();
        assert_eq!(user_info,UserInfo{
            address:"buyer1".to_string(),
            total_token:Uint128::new(4000000),
            received_token:Uint128::new(0),
            last_received_time:env.block.time.seconds()+115
        });

        let info = mock_info("buyer2",&[Coin{
            denom:"uusd".to_string(),
            amount:Uint128::new(50000)
        }]);
        let msg = ExecuteMsg::BuyToken { amount: Uint128::new(5000000) };
        let res =  execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        
        let user_info = query_user_info(deps.as_ref(), "buyer1".to_string() ).unwrap();
        assert_eq!(user_info,UserInfo{
            address:"buyer1".to_string(),
            total_token:Uint128::new(4000000),
            received_token:Uint128::new(0),
            last_received_time:env.block.time.seconds()+115
        });

        let user_info = query_user_info(deps.as_ref(), "buyer2".to_string() ).unwrap();
        assert_eq!(user_info,UserInfo{
            address:"buyer2".to_string(),
            total_token:Uint128::new(5000000),
            received_token:Uint128::new(0),
            last_received_time:env.block.time.seconds()+115
        });

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_sold_amount,Uint128::new(9000000));

        assert_eq!(res.messages.len(),1);
        assert_eq!(res.messages[0].msg,
            CosmosMsg::Bank(
            BankMsg::Send { 
                to_address: "admin".to_string(), 
                amount: vec![Coin{
                    denom:"uusd".to_string(),
                    amount:Uint128::new(50000)
                } ]
            }));

        let users = query_get_users(deps.as_ref()).unwrap();
        assert_eq!(users,vec!["buyer1","buyer2"]);
    }


    #[test]

    fn withdraw_token() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds()-240,
            presale_end:env.block.time.seconds()-10,
            total_supply:Uint128::new(1000),
            vesting_period:500,
            vesting_step_period:125,
            token_price:Uint128::new(1),
            denom:"uusd".to_string(),
             admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetTokenAddress  { address:"token_address1".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_address,"token_address1".to_string());

        let info = mock_info("buyer1",&[Coin{
            denom:"uusd".to_string(),
            amount:Uint128::new(100)
        }]);

        let msg = ExecuteMsg::BuyToken { amount: Uint128::new(100) };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("buyer1",&[]);
        let msg = ExecuteMsg::WithdrawToken {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.messages[0].msg,
           CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer { 
                recipient: "buyer1".to_string(), 
                amount: Uint128::new(20) }).unwrap(),
            funds: vec![],
        }));
        
        let user_info = query_user_info(deps.as_ref(), "buyer1".to_string()).unwrap();
           assert_eq!(user_info,UserInfo{
            address:"buyer1".to_string(),
            total_token:Uint128::new(100),
            received_token:Uint128::new(20),
            last_received_time:env.block.time.seconds()-10
        });
       
    }

    
    #[test]

    fn withdraw_admin_token() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let instantiate_msg = InstantiateMsg {
            presale_start:env.block.time.seconds()-240,
            presale_end:env.block.time.seconds()-10,
            total_supply:Uint128::new(1000),
            vesting_period:500,
            vesting_step_period:125,
            token_price:Uint128::new(1),
            denom:"uusd".to_string(),
             admin_wallet:"admin".to_string()
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.owner,"creator".to_string());

        let info = mock_info("creator", &[]);
        let msg = ExecuteMsg::SetTokenAddress  { address:"token_address1".to_string()};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let state = query_state_info(deps.as_ref()).unwrap();
        assert_eq!(state.token_address,"token_address1".to_string());

        let info = mock_info("buyer1",&[Coin{
            denom:"uusd".to_string(),
            amount:Uint128::new(100)
        }]);

        let msg = ExecuteMsg::BuyToken { amount: Uint128::new(100) };
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("admin",&[]);
        let msg = ExecuteMsg::WithdrawAdminToken  {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(res.messages[0].msg,
           CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer { 
                recipient: "admin".to_string(), 
                amount: Uint128::new(900) }).unwrap(),
            funds: vec![],
        }));
        
      
       
    }
}
    
  