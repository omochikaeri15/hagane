use crate::Fault;

use super::{Enigma, EnigmaLottery};

pub fn enigma_lottery_at(enigma: &Enigma, index: i32) -> Result<&EnigmaLottery, Fault> {
    enigma
        .lotteries
        .get(index as i64 as usize)
        .ok_or(Fault::out_of_range())
}
