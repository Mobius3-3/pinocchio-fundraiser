use crate::utils::{impl_len, impl_load};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Contributor {
    pub amount: [u8; 8],
    pub bump: u8,
}

impl_len!(Contributor);
impl_load!(Contributor);

impl Contributor {
    pub fn initialize(
        &mut self,
        amount: [u8; 8],
        bump: u8,
    ) {
        self.amount = amount;
        self.bump = bump;
    }
}