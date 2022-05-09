use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Wrong Portion Error")]
    WrongPortionError {},
  
    #[error("Not enough funds")]
    NotEnoughFunds {},
    
    #[error("Insufficient remaining token")]
    InsufficientRemainingToken {},

    #[error("Presale is not started")]
    PresaleNotStarted {},

     #[error("Presale is not finished")]
    PresaleNotFinished {},

     #[error("Not remaining token at the moment")]
    NotRemainingToken {},

    #[error("Not deposited")]
    NotDeposited {},

    #[error("Wrong setting time")]
    WrongSetTime {},
    
}
