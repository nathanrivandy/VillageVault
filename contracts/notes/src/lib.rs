#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    symbol_short, Env, String, Symbol, Vec,
};

const PROGRAM_KEY: Symbol = symbol_short!("PROGRAM");

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VillageVaultError {
    ProgramAlreadyExists      = 1,
    ProgramNotFound           = 2,
    InvalidAmount             = 3,
    InsufficientBalance       = 4,
    CannotCancelActiveProgram = 5,
    ArithmeticOverflow        = 6,
}

#[contracttype]
#[derive(Clone, Eq, PartialEq)]
pub struct Expense {
    pub id: u32,
    pub description: String,
    pub amount: i128,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone, Eq, PartialEq)]
pub struct Program {
    pub name: String,
    pub target_amount: i128,
    pub total_funds_collected: i128,
    pub remaining_balance: i128,
    pub expenses: Vec<Expense>,
}

fn program_exists(env: &Env) -> bool {
    env.storage().instance().has(&PROGRAM_KEY)
}

fn load_program(env: &Env) -> Result<Program, VillageVaultError> {
    env.storage()
        .instance()
        .get(&PROGRAM_KEY)
        .ok_or(VillageVaultError::ProgramNotFound)
}

fn save_program(env: &Env, program: &Program) {
    env.storage().instance().set(&PROGRAM_KEY, program);
}

#[contract]
pub struct VillageVault;

#[contractimpl]
impl VillageVault {
    pub fn init_program(
        env: Env,
        name: String,
        target_amount: i128,
    ) -> Result<Program, VillageVaultError> {
        if program_exists(&env) {
            return Err(VillageVaultError::ProgramAlreadyExists);
        }

        if target_amount <= 0 {
            return Err(VillageVaultError::InvalidAmount);
        }

        let program = Program {
            name,
            target_amount,
            total_funds_collected: 0,
            remaining_balance:     0,
            expenses:              Vec::new(&env),
        };

        save_program(&env, &program);
        Ok(program)
    }

    pub fn get_program_details(env: Env) -> Result<Program, VillageVaultError> {
        load_program(&env)
    }

    pub fn fund_program(
        env: Env,
        amount: i128,
    ) -> Result<Program, VillageVaultError> {
        if amount <= 0 {
            return Err(VillageVaultError::InvalidAmount);
        }

        let mut program = load_program(&env)?;

        program.total_funds_collected = program
            .total_funds_collected
            .checked_add(amount)
            .ok_or(VillageVaultError::ArithmeticOverflow)?;

        program.remaining_balance = program
            .remaining_balance
            .checked_add(amount)
            .ok_or(VillageVaultError::ArithmeticOverflow)?;

        save_program(&env, &program);
        Ok(program)
    }

    pub fn record_expense(
        env: Env,
        description: String,
        amount: i128,
    ) -> Result<Program, VillageVaultError> {
        if amount <= 0 {
            return Err(VillageVaultError::InvalidAmount);
        }

        let mut program = load_program(&env)?;

        if amount > program.remaining_balance {
            return Err(VillageVaultError::InsufficientBalance);
        }

        let expense_id = program
            .expenses
            .len()
            .checked_add(1)
            .ok_or(VillageVaultError::ArithmeticOverflow)?;

        let expense = Expense {
            id:          expense_id,
            description,
            amount,
            recorded_at: env.ledger().timestamp(),
        };

        program.expenses.push_back(expense);

        program.remaining_balance = program
            .remaining_balance
            .checked_sub(amount)
            .ok_or(VillageVaultError::ArithmeticOverflow)?;

        save_program(&env, &program);
        Ok(program)
    }

    pub fn cancel_program(env: Env) -> Result<(), VillageVaultError> {
        let program = load_program(&env)?;

        if program.total_funds_collected != 0 || !program.expenses.is_empty() {
            return Err(VillageVaultError::CannotCancelActiveProgram);
        }

        env.storage().instance().remove(&PROGRAM_KEY);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    fn setup() -> (Env, VillageVaultClient<'static>) {
        let env = Env::default();
        let contract_id = env.register(VillageVault, ());
        let client = VillageVaultClient::new(&env, &contract_id);
        (env, client)
    }

    #[test]
    fn test_init_program_success() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Clean Water Access — District 5");

        let program = client.init_program(&name, &1_000_000).unwrap();

        assert_eq!(program.name, name);
        assert_eq!(program.target_amount, 1_000_000);
        assert_eq!(program.total_funds_collected, 0);
        assert_eq!(program.remaining_balance, 0);
        assert!(program.expenses.is_empty());
    }

    #[test]
    fn test_init_program_duplicate_returns_error() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Community Road Repair Fund");

        client.init_program(&name, &500_000).unwrap();

        let result = client.init_program(&name, &500_000);
        assert_eq!(result, Err(VillageVaultError::ProgramAlreadyExists));
    }

    #[test]
    fn test_init_program_zero_target_returns_error() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Invalid Program Zero Target");

        let result = client.init_program(&name, &0);
        assert_eq!(result, Err(VillageVaultError::InvalidAmount));
    }

    #[test]
    fn test_init_program_negative_target_returns_error() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Invalid Program Negative Target");

        let result = client.init_program(&name, &-500);
        assert_eq!(result, Err(VillageVaultError::InvalidAmount));
    }

    #[test]
    fn test_get_program_details_success() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Village Hall Renovation Fund");

        client.init_program(&name, &2_000_000).unwrap();

        let program = client.get_program_details().unwrap();
        assert_eq!(program.name, name);
        assert_eq!(program.target_amount, 2_000_000);
    }

    #[test]
    fn test_get_program_details_not_found() {
        let (_env, client) = setup();

        let result = client.get_program_details();
        assert_eq!(result, Err(VillageVaultError::ProgramNotFound));
    }

    #[test]
    fn test_fund_program_success() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Community Scholarship Fund");

        client.init_program(&name, &5_000_000).unwrap();
        let program = client.fund_program(&2_000_000).unwrap();

        assert_eq!(program.total_funds_collected, 2_000_000);
        assert_eq!(program.remaining_balance, 2_000_000);
    }

    #[test]
    fn test_fund_program_multiple_deposits_accumulate() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Neighbourhood Levy Fund");

        client.init_program(&name, &1_000_000).unwrap();
        client.fund_program(&300_000).unwrap();
        client.fund_program(&200_000).unwrap();
        client.fund_program(&100_000).unwrap();

        let program = client.get_program_details().unwrap();
        assert_eq!(program.total_funds_collected, 600_000);
        assert_eq!(program.remaining_balance, 600_000);
    }

    #[test]
    fn test_fund_program_invalid_amount() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Any Active Program");

        client.init_program(&name, &500_000).unwrap();

        assert_eq!(client.fund_program(&0),    Err(VillageVaultError::InvalidAmount));
        assert_eq!(client.fund_program(&-500), Err(VillageVaultError::InvalidAmount));
    }

    #[test]
    fn test_fund_program_not_found() {
        let (_env, client) = setup();

        let result = client.fund_program(&100_000);
        assert_eq!(result, Err(VillageVaultError::ProgramNotFound));
    }

    #[test]
    fn test_record_expense_success() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Health Post Equipment Fund");
        let desc = String::from_str(&env, "Purchased 10 plastic chairs for waiting area");

        client.init_program(&name, &1_000_000).unwrap();
        client.fund_program(&800_000).unwrap();

        let program = client.record_expense(&desc, &150_000).unwrap();

        assert_eq!(program.expenses.len(), 1);
        assert_eq!(program.remaining_balance, 650_000);

        let expense = program.expenses.get(0).unwrap();
        assert_eq!(expense.id, 1);
        assert_eq!(expense.amount, 150_000);
        assert_eq!(expense.description, desc);
    }

    #[test]
    fn test_record_multiple_expenses_auto_increment_ids() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Street Painting Initiative");

        client.init_program(&name, &1_000_000).unwrap();
        client.fund_program(&1_000_000).unwrap();

        let d1 = String::from_str(&env, "Purchased 20 litres of white exterior paint");
        let d2 = String::from_str(&env, "Purchased brushes and paint rollers");
        let d3 = String::from_str(&env, "Rented scaffolding for 3 days");

        client.record_expense(&d1, &200_000).unwrap();
        client.record_expense(&d2, &100_000).unwrap();
        client.record_expense(&d3, &150_000).unwrap();

        let program = client.get_program_details().unwrap();
        assert_eq!(program.expenses.len(), 3);
        assert_eq!(program.expenses.get(0).unwrap().id, 1);
        assert_eq!(program.expenses.get(1).unwrap().id, 2);
        assert_eq!(program.expenses.get(2).unwrap().id, 3);
        assert_eq!(program.remaining_balance, 550_000);
    }

    #[test]
    fn test_record_expense_insufficient_balance() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Low Balance Program");
        let desc = String::from_str(&env, "Expense exceeding available funds");

        client.init_program(&name, &500_000).unwrap();
        client.fund_program(&100_000).unwrap();

        let result = client.record_expense(&desc, &200_000);
        assert_eq!(result, Err(VillageVaultError::InsufficientBalance));
    }

    #[test]
    fn test_record_expense_invalid_amount() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Any Funded Program");
        let desc = String::from_str(&env, "Zero or negative expense attempt");

        client.init_program(&name, &500_000).unwrap();
        client.fund_program(&500_000).unwrap();

        assert_eq!(client.record_expense(&desc, &0),    Err(VillageVaultError::InvalidAmount));
        assert_eq!(client.record_expense(&desc, &-100), Err(VillageVaultError::InvalidAmount));
    }

    #[test]
    fn test_record_expense_not_found() {
        let (env, client) = setup();
        let desc = String::from_str(&env, "Expense on non-existent program");

        let result = client.record_expense(&desc, &50_000);
        assert_eq!(result, Err(VillageVaultError::ProgramNotFound));
    }

    #[test]
    fn test_cancel_program_success() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Cancelled Before Launch");

        client.init_program(&name, &300_000).unwrap();
        assert_eq!(client.cancel_program(), Ok(()));

        assert_eq!(
            client.get_program_details(),
            Err(VillageVaultError::ProgramNotFound)
        );
    }

    #[test]
    fn test_cancel_program_blocked_after_funding() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Already Funded Program");

        client.init_program(&name, &500_000).unwrap();
        client.fund_program(&100_000).unwrap();

        let result = client.cancel_program();
        assert_eq!(result, Err(VillageVaultError::CannotCancelActiveProgram));
    }

    #[test]
    fn test_cancel_program_blocked_after_expense() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Program With Recorded Expenses");
        let desc = String::from_str(&env, "Purchased office stationery");

        client.init_program(&name, &500_000).unwrap();
        client.fund_program(&500_000).unwrap();
        client.record_expense(&desc, &50_000).unwrap();

        let result = client.cancel_program();
        assert_eq!(result, Err(VillageVaultError::CannotCancelActiveProgram));
    }

    #[test]
    fn test_cancel_program_not_found() {
        let (_env, client) = setup();

        let result = client.cancel_program();
        assert_eq!(result, Err(VillageVaultError::ProgramNotFound));
    }

    #[test]
    fn test_full_lifecycle() {
        let (env, client) = setup();
        let name = String::from_str(&env, "Community Mosque Renovation — Block 3");

        let program = client.init_program(&name, &10_000_000).unwrap();
        assert_eq!(program.remaining_balance, 0);

        client.fund_program(&4_000_000).unwrap();
        client.fund_program(&3_000_000).unwrap();

        let program = client.get_program_details().unwrap();
        assert_eq!(program.total_funds_collected, 7_000_000);
        assert_eq!(program.remaining_balance, 7_000_000);

        let d1 = String::from_str(&env, "Purchased 100 sacks of cement");
        let d2 = String::from_str(&env, "Labour wages for 2 weeks");
        let d3 = String::from_str(&env, "Purchased floor tiles");

        client.record_expense(&d1, &2_000_000).unwrap();
        client.record_expense(&d2, &1_500_000).unwrap();
        client.record_expense(&d3, &1_200_000).unwrap();

        let program = client.get_program_details().unwrap();
        assert_eq!(program.expenses.len(), 3);
        assert_eq!(program.total_funds_collected, 7_000_000);
        assert_eq!(program.remaining_balance, 2_300_000);

        assert_eq!(
            client.cancel_program(),
            Err(VillageVaultError::CannotCancelActiveProgram)
        );
    }
}