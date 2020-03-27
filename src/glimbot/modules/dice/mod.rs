use rand::distributions::Uniform;
use rand::prelude::Distribution;
use thiserror::Error;
use circular_queue::CircularQueue;

mod module;
pub use module::*;
mod parser;

const MAX_DICE_PER_ROLL: usize = 100000;
const MAX_TRACKED_DICE: usize = MAX_DICE_PER_ROLL / 10;
const MAX_DICE_FOR_DISPLAY: usize = 20;

#[derive(Debug, Clone, Copy)]
pub enum RollComponent {
    Dice { num_dice: u32, die_type: u32 },
    Constant(u32)
}

#[derive(Debug, Clone)]
pub struct RollResult {
    sum: i64,
    rolls: CircularQueue<u32>
}

impl RollResult {
    pub fn new() -> RollResult {
        RollResult { sum: 0, rolls: CircularQueue::with_capacity(MAX_TRACKED_DICE) }
    }

    pub fn add_roll(&mut self, roll: u32) {
        self.sum = self.sum.saturating_add(roll as i64);
        self.rolls.push(roll);
    }

    pub fn add_const(&mut self, val: u32) {
        self.sum = self.sum.saturating_add(val as i64);
    }

    pub fn add(mut self, res: RollResult) -> RollResult {
        self.sum = self.sum.saturating_add(res.sum);
        res.rolls.iter().for_each(|i| self.rolls.push(*i));
        self
    }

    pub fn sub(mut self, res: RollResult) -> RollResult {
        self.sum = self.sum.saturating_sub(res.sum);
        res.rolls.iter().for_each(|i| self.rolls.push(*i));
        self
    }

    pub fn avg(&self) -> Option<f64> {
        if !self.rolls.is_empty() {
            let tot: f64 = self.rolls.iter().map(|i| *i as f64).sum();
            Some(tot / self.rolls.len() as f64)
        } else {
            None
        }
    }


}

impl RollComponent {
    pub fn eval(&self) -> RollResult {
        let mut out = RollResult::new();
        match self {
            RollComponent::Dice { num_dice, die_type } => {
                let mut rng = rand::thread_rng();
                let dist = Uniform::new(1u32, *die_type as u32 + 1);
                (0..*num_dice).map(|_| dist.sample(&mut rng)).for_each(|r| out.add_roll(r));
            },
            RollComponent::Constant(u) => {out.add_const(*u)},
        };

        out
    }

    pub fn constant(val: u32) -> RollComponent {
        RollComponent::Constant(val)
    }

    pub fn die(num_dice: u32, die_type: u32) -> RollComponent {
        RollComponent::Dice {num_dice, die_type}
    }

    pub fn num_dice(&self) -> usize {
        match self {
            RollComponent::Dice { num_dice, .. } => {*num_dice as usize},
            RollComponent::Constant(_) => {0},
        }
    }
}

#[derive(Debug, Clone)]
pub enum Roll {
    Add(Box<Roll>, Box<Roll>),
    Sub(Box<Roll>, Box<Roll>),
    Atom(RollComponent)
}

impl From<RollComponent> for Roll {
    fn from(r: RollComponent) -> Self {
        Roll::Atom(r)
    }
}

#[derive(Debug, Error)]
pub enum InvalidRoll {
    #[error("Too many dice in the roll!")]
    TooManyDice
}


impl Roll {
    pub fn valid(&self) -> Result<(), InvalidRoll> {
        if self.num_dice() > MAX_DICE_PER_ROLL {
            Err(InvalidRoll::TooManyDice)
        } else {
            Ok(())
        }
    }

    pub fn num_dice(&self) -> usize {
        match self {
            Roll::Add(l, r) => {l.num_dice().saturating_add(r.num_dice())},
            Roll::Sub(l, r) => {l.num_dice().saturating_add(r.num_dice())},
            Roll::Atom(a) => {a.num_dice()},
        }
    }

    pub fn add(l: impl Into<Roll>, r: impl Into<Roll>) -> Roll {
        Roll::Add(Box::new(l.into()), Box::new(r.into()))
    }

    pub fn sub(l: impl Into<Roll>, r: impl Into<Roll>) -> Roll {
        Roll::Sub(Box::new(l.into()), Box::new(r.into()))
    }

    pub fn eval(&self) -> RollResult {
        match self {
            Roll::Add(l, r) => {
                l.eval().add(r.eval())
            },
            Roll::Sub(l, r) => {
                l.eval().sub(r.eval())
            },
            Roll::Atom(d) => {
                d.eval()
            },
        }
    }
}

impl std::fmt::Display for RollResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut lines = Vec::new();
        lines.push(format!("Total: {}", self.sum));
        if !self.rolls.is_empty() {
            lines.push(format!("Rolls (up to last {} dice): {:?}", MAX_DICE_FOR_DISPLAY, self.rolls.iter().take(MAX_DICE_FOR_DISPLAY).collect::<Vec<_>>()));
            lines.push(format!("Average Roll (up to {} dice): {}", MAX_TRACKED_DICE, self.avg().unwrap()));
        }

        write!(f, "{}", lines.join("\n"))
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval() {
        let expr = Roll::add(RollComponent::constant(10), RollComponent::die(10, 20));
        println!("{}", expr.eval());
    }

    #[test]
    fn test_validation() {
        let expr = parser::parse_roll("100d10");
        assert!(expr.unwrap().eval().sum > 100);

        let failed = parser::parse_roll("1d3 + 10000000d5");
        println!("{:?}", &failed);
        assert!(failed.unwrap().valid().is_err())
    }
}