use crate::utils::{impl_len, impl_load};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Fundraiser {
    pub maker: [u8; 32],
    pub mint_to_raise: [u8; 32],
    pub amount_to_raise: [u8; 8],
    pub current_amount: [u8; 8],
    pub time_started: [u8; 8],
    pub duration: u8,
    pub bump: u8,
}

impl_len!(Fundraiser);
impl_load!(Fundraiser);

impl Fundraiser {
    pub fn initialize(
        &mut self,
        maker: [u8; 32],
        mint_to_raise: [u8; 32],
        amount_to_raise: [u8; 8],
        current_amount: [u8; 8],
        time_started: [u8; 8],
        duration: u8,
        bump: u8,
    ) {
        self.maker = maker;
        self.mint_to_raise = mint_to_raise;
        self.amount_to_raise = amount_to_raise;
        self.current_amount = current_amount;
        self.time_started = time_started;
        self.duration = duration;
        self.bump = bump;
    }
}