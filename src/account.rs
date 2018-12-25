use {
    chrono::naive::NaiveDate,
    self::{AccountType::*, Action::*, Inflow::*},
    std::fmt,
};

#[derive(Debug)]
pub struct Account {
    name: String,
    data: AccountType
}

#[derive(Debug)]
pub struct BranchEntry {
    account: Account,
    inflow: Inflow,
}

#[derive(Debug)]
pub enum AccountType {
    Leaf { balance: f64, max: f64 },
    Branch { children: Vec<BranchEntry> }
}

#[derive(Debug)]
pub enum Inflow  {
    Fixed(f64),
    Flex(f64)
}

pub enum Action {
    New { name: String, inflow: Inflow, parent: String, data: AccountType },
    Withdraw { account: String, amount: f64, date: NaiveDate },
    Deposit { account: Option<String>, amount: f64, date: NaiveDate },
    Transfer { from: String, to: Option<String>, amount: f64, date: NaiveDate }
}

impl Account {
    pub fn new_root() -> Account {
        Account {
            name: "root".to_owned(),
            data: Branch { children: Vec::new() }
        }
    }

    pub fn apply(&mut self, action: Action) -> Result<(), String> {
        match action {
            New { name, inflow, parent, data } => {
                let parent = self.find_child(&parent)
                    .ok_or(format!("Could not find parent account {} to create account {}", parent, name))?;
                let account = Account { name, data };
                parent.add_child(account, inflow)
            }
            Withdraw { account, amount, .. } => {
                let parent = self.find_child(&account)
                    .ok_or(format!("Could not find parent account {} to withdraw from", account))?;
                parent.withdraw(amount)?;
                Ok(())
            }
            Deposit { account, amount, .. } => {
                let account = match account {
                    Some(parent) => self.find_child(&parent)
                        .ok_or(format!("Could not find parent account {} to deposit to", parent))?,
                    None => self
                };
                account.deposit(amount);
                Ok(())
            }
            Transfer { from, to, amount, date } => {
                self.apply(Action::Withdraw { account: from, amount, date })?;
                self.apply(Action::Deposit { account: to, amount, date })
            }
        }
    }

    pub fn balance(&self) -> f64 {
        match self.data {
            Leaf { balance, .. } => balance,
            Branch { ref children } => children
                .iter()
                .map(|BranchEntry { account, .. }| account.balance())
                .sum()
        }
    }

    pub fn deposit(&mut self, amount: f64) {
        match self.data {
            Leaf { ref mut balance, .. } => *balance += amount,
            Branch { ref mut children } => {
                // Make fixed deposits
                let mut amount = children.iter_mut()
                    .fold(amount, |amount, child| child.make_fixed_deposit(amount));
                // Make flex deposits
                let mut total_flex: f64 = children.iter().map(BranchEntry::get_flex).sum();
                while total_flex != 0.0 && amount > 0.01 {
                    let per_flex = amount / total_flex;
                    amount = children.iter_mut()
                        .fold(amount, |amount, child| child.make_flex_deposit(amount, per_flex));
                    total_flex = children.iter().map(BranchEntry::get_flex).sum();
                }
                // Give up and redistribute
                let remaining = amount / children.len() as f64;
                if remaining > 0.01 {
                    children.iter_mut().for_each(|child| child.account.deposit(remaining));
                }
            }
        }
    }

    pub fn withdraw(&mut self, amount: f64) -> Result<(), String> {
        match self.data {
            Leaf { ref mut balance, .. } => Ok(*balance -= amount),
            _ => Err("Cannot withdraw from a branch node".to_owned())
        }
    }

    pub fn find_child(&mut self, name: &str) -> Option<&mut Account> {
        if self.name == name {
            return Some(self);
        }
        match &mut self.data {
            Leaf { .. } => None,
            Branch { children } =>  {
                for child in children.iter_mut() {
                    if child.account.name == name {
                        return Some(&mut child.account)
                    } else {
                        if let Some(account) = child.account.find_child(name) {
                            return Some(account);
                        }
                    }
                }
                None
            }
        }
    }

    pub fn add_child(&mut self, account: Account, inflow: Inflow) -> Result<(), String> {
        match &mut self.data {
            Leaf { .. } => Err("Cannot add a child to a leaf account".to_owned()),
            Branch { children } => {
                children.push(BranchEntry { account, inflow });
                Ok(())
            }
        }
    }

    fn print_level(&self, f: &mut fmt::Formatter, level: u32) -> fmt::Result {
        for _ in 0..level {
            print!("  ");
        }
        println!("{}: {:.2}", self.name, self.balance());
        match &self.data {
            Leaf {..}  => Ok(()),
            Branch { children } => {
                for child in children {
                    child.account.print_level(f, level + 1)?
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.print_level(f, 0)
    }
}

impl BranchEntry {
    fn max(&self) -> f64 {
        match &self.account.data {
            Leaf { max, .. } => *max,
            Branch { children } => children.iter().map(BranchEntry::max).sum()
        }
    }

    fn until_max(&self) -> f64 {
        self.max() - self.account.balance()
    }

    fn at_max(&self) -> bool {
        self.until_max() <= 0.0
    }

    fn get_flex(&self) -> f64 {
        match self.inflow {
            Fixed(_) => 0.0,
            Flex(_) if self.at_max() => 0.0,
            Flex(x) => x
        }
    }

    fn make_fixed_deposit(&mut self, available: f64) -> f64 {
        match self.inflow {
            Fixed(take) => {
                let take = take.min(self.until_max()).min(available);
                self.account.deposit(take);
                available - take
            }
            Flex(_) => available
        }
    }

    fn make_flex_deposit(&mut self, available: f64, per_flex: f64) -> f64 {
        match self.inflow {
            Flex(flex) => {
                let take = (per_flex * flex).min(available).min(self.until_max());
                self.account.deposit(take);
                available - take
            },
            _ => available
        }
    }
}
