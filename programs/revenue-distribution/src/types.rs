use std::fmt::Display;

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;

#[derive(
    Debug,
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Pod,
    Zeroable,
)]
#[repr(C)]
pub struct DoubleZeroEpoch(u64);

impl DoubleZeroEpoch {
    pub fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn as_seed(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    pub fn saturating_add_duration(&self, epoch_duration: EpochDuration) -> Self {
        Self(self.0.saturating_add(epoch_duration.into()))
    }
}

impl Display for DoubleZeroEpoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u64> for DoubleZeroEpoch {
    fn eq(&self, rhs: &u64) -> bool {
        self.0 == *rhs
    }
}

/// Any calculation requiring the passage of time via DoubleZero epochs as an input should use this
/// type. `u32::MAX` is more than enough time for any of these calculations.
pub type EpochDuration = u32;

pub type ValidatorFee = UnitShare16;
pub type BurnRate = UnitShare32;

/// Macro to implement common UnitShare functionality for different integer types.
macro_rules! impl_unit_share {
    ($name:ident, $inner_type:ty, $max_value:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
        #[repr(C)]
        pub struct $name($inner_type);

        impl Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}/{}", self.0, Self::MAX.0)
            }
        }

        impl $name {
            pub const MIN: Self = Self(0);
            pub const MAX: Self = Self($max_value);

            pub const fn new(value: $inner_type) -> Option<Self> {
                if value <= Self::MAX.0 {
                    Some(Self(value))
                } else {
                    None
                }
            }

            pub fn mul_scalar<T>(&self, x: T) -> T
            where
                T: Into<u128> + TryFrom<u128>,
                <T as TryFrom<u128>>::Error: std::fmt::Debug,
            {
                let result = u128::from(self.0)
                    .saturating_mul(x.into())
                    .saturating_div(Self::MAX.0.into());

                result
                    .try_into()
                    .expect("mul_scalar result should fit in target type")
            }

            pub fn mul_scalar_rounded<T>(&self, x: T) -> T
            where
                T: Into<u128> + TryFrom<u128>,
                <T as TryFrom<u128>>::Error: std::fmt::Debug,
            {
                let numerator = u128::from(self.0).saturating_mul(x.into());
                let denominator = Self::MAX.0.into();

                let result = numerator
                    .saturating_add(denominator / 2)
                    .saturating_div(denominator);

                result
                    .try_into()
                    .expect("mul_scalar_rounded result should fit in target type")
            }

            pub fn checked_add(&self, other: Self) -> Option<Self> {
                let value = self.0.checked_add(other.0)?;

                if value <= Self::MAX.0 {
                    Some(Self(value))
                } else {
                    None
                }
            }

            pub fn checked_sub(&self, other: Self) -> Option<Self> {
                let value = self.0.checked_sub(other.0)?;
                // Value is guaranteed to be <= self.0 <= Self::MAX.0, so no bounds check needed.
                Some(Self(value))
            }

            pub fn saturating_add(&self, other: Self) -> Self {
                Self(self.0.saturating_add(other.0)).min(Self::MAX)
            }

            pub fn saturating_sub(&self, other: Self) -> Self {
                Self(self.0.saturating_sub(other.0))
            }
        }

        impl From<$name> for $inner_type {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                u64::from(value.0)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = &'static str;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                let inner_value: $inner_type = value
                    .try_into()
                    .map_err(|_| "Value too large for inner type")?;
                Self::new(inner_value).ok_or("Value exceeds maximum allowed")
            }
        }
    };
}

impl_unit_share!(
    UnitShare16,
    u16,
    10_000,
    "A 16-bit unit share type with maximum value 10,000 (e.g., 420 is 4.20%)."
);

impl_unit_share!(
    UnitShare32,
    u32,
    1_000_000_000,
    "A 32-bit unit share type with maximum value 1,000,000,000 (e.g., 420,000,069 is 42.0000069%)."
);

#[derive(
    Debug, BorshDeserialize, BorshSerialize, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable,
)]
#[repr(C)]
pub struct SolanaValidatorDebt {
    pub node_id: Pubkey,
    pub amount: u64,
}

impl SolanaValidatorDebt {
    pub const LEAF_PREFIX: &'static [u8] = b"solana_validator_debt";
}

#[derive(
    Debug, BorshDeserialize, BorshSerialize, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable,
)]
#[repr(C)]
pub struct RewardShare {
    pub contributor_key: Pubkey,
    pub unit_share: u32,
    pub remaining_bytes: [u8; 4],
}

impl RewardShare {
    pub const LEAF_PREFIX: &'static [u8] = b"reward_share";

    pub const FLAG_IS_BLOCKED_BIT: usize = 31;
    pub const FLAG_IS_BLOCKED_MASK: u32 = 1 << Self::FLAG_IS_BLOCKED_BIT;
    pub const ECONOMIC_BURN_RATE_MASK: u32 = 0x3FFFFFFF;

    pub fn new(
        contributor_key: Pubkey,
        unit_share: u32,
        should_block: bool,
        economic_burn_rate: u32,
    ) -> Option<Self> {
        // Check that the rates are valid.
        let unit_share = UnitShare32::new(unit_share)?;
        let economic_burn_rate = UnitShare32::new(economic_burn_rate)?;

        // Start with the economic burn rate (first 30 bits).
        let mut combined_value = economic_burn_rate.0;

        // Set the blocked flag.
        if should_block {
            combined_value |= Self::FLAG_IS_BLOCKED_MASK;
        }

        Some(Self {
            contributor_key,
            unit_share: unit_share.0,
            remaining_bytes: combined_value.to_le_bytes(),
        })
    }

    pub fn checked_unit_share(&self) -> Option<UnitShare32> {
        UnitShare32::new(self.unit_share)
    }

    pub fn is_blocked(&self) -> bool {
        let combined_value = u32::from_le_bytes(self.remaining_bytes);
        combined_value & Self::FLAG_IS_BLOCKED_MASK != 0
    }

    pub fn set_is_blocked(&mut self, should_block: bool) {
        let mut combined_value = u32::from_le_bytes(self.remaining_bytes);
        if should_block {
            combined_value |= Self::FLAG_IS_BLOCKED_MASK;
        } else {
            combined_value &= !Self::FLAG_IS_BLOCKED_MASK;
        }
        self.remaining_bytes = combined_value.to_le_bytes();
    }

    pub fn economic_burn_rate(&self) -> u32 {
        let combined_value = u32::from_le_bytes(self.remaining_bytes);
        combined_value & Self::ECONOMIC_BURN_RATE_MASK
    }

    pub fn checked_economic_burn_rate(&self) -> Option<UnitShare32> {
        UnitShare32::new(self.economic_burn_rate())
    }

    pub fn set_economic_burn_rate(&mut self, economic_burn_rate: UnitShare32) {
        let mut combined_value = u32::from_le_bytes(self.remaining_bytes);
        combined_value &= !Self::ECONOMIC_BURN_RATE_MASK;
        combined_value |= economic_burn_rate.0;
        self.remaining_bytes = combined_value.to_le_bytes();
    }
}

/// A byte wrapper for bit flag operations. Each bit can be individually set or
/// checked. This type can be used for both flags and replay protection.
#[derive(
    Debug, BorshDeserialize, BorshSerialize, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable,
)]
#[repr(C)]
pub struct ByteFlags(u8);

impl ByteFlags {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn bit(&self, index: usize) -> bool {
        if index >= 8 {
            false
        } else {
            (self.0 & (1 << index)) != 0
        }
    }

    pub fn set_bit(&mut self, index: usize, value: bool) {
        if index < 8 {
            if value {
                self.0 |= 1 << index;
            } else {
                self.0 &= !(1 << index);
            }
        }
    }
}

impl From<ByteFlags> for u8 {
    fn from(value: ByteFlags) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_share16_constants() {
        assert_eq!(UnitShare16::MIN.0, 0);
        assert_eq!(UnitShare16::MAX.0, 10_000);
    }

    #[test]
    fn test_unit_share32_constants() {
        assert_eq!(UnitShare32::MIN.0, 0);
        assert_eq!(UnitShare32::MAX.0, 1_000_000_000);
    }

    #[test]
    fn test_unit_share16_new() {
        assert_eq!(UnitShare16::new(0).unwrap(), UnitShare16::MIN);
        assert_eq!(UnitShare16::new(5_000).unwrap(), UnitShare16(5_000));
        assert_eq!(UnitShare16::new(10_000).unwrap(), UnitShare16::MAX);
        assert!(UnitShare16::new(10_001).is_none());
        assert!(UnitShare16::new(u16::MAX).is_none());
    }

    #[test]
    fn test_unit_share32_new() {
        assert_eq!(UnitShare32::new(0).unwrap(), UnitShare32::MIN);
        assert_eq!(
            UnitShare32::new(500_000_000).unwrap(),
            UnitShare32(500_000_000)
        );
        assert_eq!(UnitShare32::new(1_000_000_000).unwrap(), UnitShare32::MAX);
        assert!(UnitShare32::new(1_000_000_001).is_none());
        assert!(UnitShare32::new(u32::MAX).is_none());
    }

    #[test]
    fn test_unit_share16_display() {
        assert_eq!(format!("{}", UnitShare16(0)), "0/10000");
        assert_eq!(format!("{}", UnitShare16(5_000)), "5000/10000");
        assert_eq!(format!("{}", UnitShare16::MAX), "10000/10000");
    }

    #[test]
    fn test_unit_share32_display() {
        assert_eq!(format!("{}", UnitShare32(0)), "0/1000000000");
        assert_eq!(
            format!("{}", UnitShare32(500_000_000)),
            "500000000/1000000000"
        );
        assert_eq!(format!("{}", UnitShare32::MAX), "1000000000/1000000000");
    }

    #[test]
    fn test_unit_share16_checked_add() {
        let a = UnitShare16(3_000);
        let b = UnitShare16(2_000);
        let c = UnitShare16(8_000);

        assert_eq!(a.checked_add(b).unwrap(), UnitShare16(5_000));
        assert!(a.checked_add(c).is_none()); // 3000 + 8000 = 11000 > MAX.
        assert!(UnitShare16::MAX.checked_add(UnitShare16(1)).is_none());
        assert_eq!(
            UnitShare16::MIN.checked_add(UnitShare16::MAX).unwrap(),
            UnitShare16::MAX
        );
    }

    #[test]
    fn test_unit_share32_checked_add() {
        let a = UnitShare32(300_000_000);
        let b = UnitShare32(200_000_000);
        let c = UnitShare32(800_000_000);

        assert_eq!(a.checked_add(b).unwrap(), UnitShare32(500_000_000));
        assert!(a.checked_add(c).is_none()); // Would exceed MAX.
        assert!(UnitShare32::MAX.checked_add(UnitShare32(1)).is_none());
        assert_eq!(
            UnitShare32::MIN.checked_add(UnitShare32::MAX).unwrap(),
            UnitShare32::MAX
        );
    }

    #[test]
    fn test_unit_share16_checked_sub() {
        let a = UnitShare16(5_000);
        let b = UnitShare16(2_000);
        let c = UnitShare16(8_000);

        assert_eq!(a.checked_sub(b).unwrap(), UnitShare16(3_000));
        assert!(a.checked_sub(c).is_none()); // 5000 - 8000 would underflow.
        assert!(UnitShare16::MIN.checked_sub(UnitShare16(1)).is_none());
        assert_eq!(
            UnitShare16::MAX.checked_sub(UnitShare16::MIN).unwrap(),
            UnitShare16::MAX
        );
    }

    #[test]
    fn test_unit_share32_checked_sub() {
        let a = UnitShare32(500_000_000);
        let b = UnitShare32(200_000_000);
        let c = UnitShare32(800_000_000);

        assert_eq!(a.checked_sub(b).unwrap(), UnitShare32(300_000_000));
        assert!(a.checked_sub(c).is_none()); // Would underflow.
        assert!(UnitShare32::MIN.checked_sub(UnitShare32(1)).is_none());
        assert_eq!(
            UnitShare32::MAX.checked_sub(UnitShare32::MIN).unwrap(),
            UnitShare32::MAX
        );
    }

    #[test]
    fn test_unit_share16_saturating_add() {
        let a = UnitShare16(3_000);
        let b = UnitShare16(2_000);
        let c = UnitShare16(8_000);

        assert_eq!(a.saturating_add(b), UnitShare16(5_000));
        assert_eq!(a.saturating_add(c), UnitShare16::MAX); // Saturates at MAX.
        assert_eq!(
            UnitShare16::MAX.saturating_add(UnitShare16(1_000)),
            UnitShare16::MAX
        );
    }

    #[test]
    fn test_unit_share32_saturating_add() {
        let a = UnitShare32(300_000_000);
        let b = UnitShare32(200_000_000);
        let c = UnitShare32(800_000_000);

        assert_eq!(a.saturating_add(b), UnitShare32(500_000_000));
        assert_eq!(a.saturating_add(c), UnitShare32::MAX); // Saturates at MAX.
        assert_eq!(
            UnitShare32::MAX.saturating_add(UnitShare32(1_000)),
            UnitShare32::MAX
        );
    }

    #[test]
    fn test_unit_share16_saturating_sub() {
        let a = UnitShare16(5_000);
        let b = UnitShare16(2_000);
        let c = UnitShare16(8_000);

        assert_eq!(a.saturating_sub(b), UnitShare16(3_000));
        assert_eq!(a.saturating_sub(c), UnitShare16::MIN); // Saturates at MIN.
        assert_eq!(
            UnitShare16::MIN.saturating_sub(UnitShare16(1_000)),
            UnitShare16::MIN
        );
    }

    #[test]
    fn test_unit_share32_saturating_sub() {
        let a = UnitShare32(500_000_000);
        let b = UnitShare32(200_000_000);
        let c = UnitShare32(800_000_000);

        assert_eq!(a.saturating_sub(b), UnitShare32(300_000_000));
        assert_eq!(a.saturating_sub(c), UnitShare32::MIN); // Saturates at MIN.
        assert_eq!(
            UnitShare32::MIN.saturating_sub(UnitShare32(1_000)),
            UnitShare32::MIN
        );
    }

    #[test]
    fn test_unit_share16_mul_scalar() {
        let half = UnitShare16(5_000); // 50%.
        let quarter = UnitShare16(2_500); // 25%.

        assert_eq!(half.mul_scalar(100_u64), 50_u64);
        assert_eq!(quarter.mul_scalar(100_u64), 25_u64);
        assert_eq!(UnitShare16::MAX.mul_scalar(100_u64), 100_u64);
        assert_eq!(UnitShare16::MIN.mul_scalar(100_u64), 0_u64);

        // Test precision.
        assert_eq!(UnitShare16(1).mul_scalar(10_000_u64), 1_u64); // 0.01% of 10000 = 1.
    }

    #[test]
    fn test_unit_share32_mul_scalar() {
        let half = UnitShare32(500_000_000); // 50%.
        let quarter = UnitShare32(250_000_000); // 25%.

        assert_eq!(half.mul_scalar(100_u64), 50_u64);
        assert_eq!(quarter.mul_scalar(100_u64), 25_u64);
        assert_eq!(UnitShare32::MAX.mul_scalar(100_u64), 100_u64);
        assert_eq!(UnitShare32::MIN.mul_scalar(100_u64), 0_u64);

        // Test high precision.
        assert_eq!(UnitShare32(1).mul_scalar(1_000_000_000_u64), 1_u64); // 0.0000001% of 1B = 1.
    }

    #[test]
    fn test_unit_share16_from_u64() {
        assert_eq!(u64::from(UnitShare16(0)), 0_u64);
        assert_eq!(u64::from(UnitShare16(5_000)), 5_000_u64);
        assert_eq!(u64::from(UnitShare16::MAX), 10_000_u64);
    }

    #[test]
    fn test_unit_share32_from_u64() {
        assert_eq!(u64::from(UnitShare32(0)), 0_u64);
        assert_eq!(u64::from(UnitShare32(500_000_000)), 500_000_000_u64);
        assert_eq!(u64::from(UnitShare32::MAX), 1_000_000_000_u64);
    }

    #[test]
    fn test_unit_share16_try_from_u64() {
        assert_eq!(UnitShare16::try_from(0_u64).unwrap(), UnitShare16::MIN);
        assert_eq!(
            UnitShare16::try_from(5_000_u64).unwrap(),
            UnitShare16(5_000)
        );
        assert_eq!(UnitShare16::try_from(10_000_u64).unwrap(), UnitShare16::MAX);

        // Test error cases.
        assert!(UnitShare16::try_from(10_001_u64).is_err());
    }

    #[test]
    fn test_unit_share32_try_from_u64() {
        assert_eq!(UnitShare32::try_from(0_u64).unwrap(), UnitShare32::MIN);
        assert_eq!(
            UnitShare32::try_from(500_000_000_u64).unwrap(),
            UnitShare32(500_000_000)
        );
        assert_eq!(
            UnitShare32::try_from(1_000_000_000_u64).unwrap(),
            UnitShare32::MAX
        );

        // Test error cases.
        assert!(UnitShare32::try_from(1_000_000_001_u64).is_err());
    }

    #[test]
    fn test_unit_share16_edge_cases() {
        // Test with maximum possible values that do not overflow u16.
        let max_minus_one = UnitShare16(9_999);
        let one = UnitShare16(1);

        assert_eq!(max_minus_one.checked_add(one).unwrap(), UnitShare16::MAX);
        assert!(max_minus_one.checked_add(UnitShare16(2)).is_none());

        // Test multiplication edge cases.
        assert_eq!(UnitShare16::MAX.mul_scalar(u64::MAX), u64::MAX);
        assert_eq!(UnitShare16::MIN.mul_scalar(u64::MAX), 0_u64);
    }

    #[test]
    fn test_unit_share32_edge_cases() {
        // Test with maximum possible values that do not overflow u32.
        let max_minus_one = UnitShare32(999_999_999);
        let one = UnitShare32(1);

        assert_eq!(max_minus_one.checked_add(one).unwrap(), UnitShare32::MAX);
        assert!(max_minus_one.checked_add(UnitShare32(2)).is_none());

        // Test multiplication edge cases.
        assert_eq!(UnitShare32::MAX.mul_scalar(u64::MAX), u64::MAX);
        assert_eq!(UnitShare32::MIN.mul_scalar(u64::MAX), 0_u64);
    }

    #[test]
    fn test_reward_share_new() {
        let contributor_key = Pubkey::new_unique();
        let unit_share = UnitShare32(500_000_000);
        let should_block = true;
        let economic_burn_rate = 100_000_000;

        let mut reward_share = RewardShare::new(
            contributor_key,
            unit_share.0,
            should_block,
            economic_burn_rate,
        )
        .unwrap();

        assert_eq!(reward_share.contributor_key, contributor_key);
        assert_eq!(reward_share.checked_unit_share().unwrap(), unit_share);
        assert_eq!(
            reward_share.checked_economic_burn_rate().unwrap(),
            UnitShare32(100_000_000)
        );
        assert!(reward_share.is_blocked());

        // Test setters.
        reward_share.set_is_blocked(false);
        assert!(!reward_share.is_blocked());

        reward_share.set_economic_burn_rate(UnitShare32(200_000_000));
        assert_eq!(
            reward_share.checked_economic_burn_rate().unwrap(),
            UnitShare32(200_000_000)
        );
    }

    #[test]
    fn test_unit_share16_mul_scalar_rounded() {
        let five_pct = UnitShare16(500); // 5%
        let half = UnitShare16(5_000); // 50%
        let quarter = UnitShare16(2_500); // 25%

        // Test cases where rounding makes no difference (exact divisions)
        assert_eq!(half.mul_scalar_rounded(100_u64), 50_u64);
        assert_eq!(quarter.mul_scalar_rounded(100_u64), 25_u64);
        assert_eq!(UnitShare16::MAX.mul_scalar_rounded(100_u64), 100_u64);
        assert_eq!(UnitShare16::MIN.mul_scalar_rounded(100_u64), 0_u64);

        // Test cases where rounding should round UP (fractional part >= 0.5)
        // 5% of 542321371 = 27116068.55, should round to 27116069
        assert_eq!(five_pct.mul_scalar_rounded(542321371_u64), 27116069_u64);

        // 5% of 542321373 = 27116068.65, should round to 27116069
        assert_eq!(five_pct.mul_scalar_rounded(542321373_u64), 27116069_u64);

        // Test cases where rounding should round DOWN (fractional part < 0.5)
        // 5% of 542321370 = 27116068.5, rounds to 27116069 (0.5 rounds up)
        assert_eq!(five_pct.mul_scalar_rounded(542321370_u64), 27116069_u64);

        // 5% of 542321369 = 27116068.45, should round to 27116068
        assert_eq!(five_pct.mul_scalar_rounded(542321369_u64), 27116068_u64);

        // Test precision with small values
        assert_eq!(UnitShare16(1).mul_scalar_rounded(10_000_u64), 1_u64); // 0.01% of 10000 = 1
        assert_eq!(UnitShare16(1).mul_scalar_rounded(5_000_u64), 1_u64); // 0.01% of 5000 = 0.5, rounds to 1
        assert_eq!(UnitShare16(1).mul_scalar_rounded(4_999_u64), 0_u64); // 0.01% of 4999 = 0.4999, rounds to 0
    }

    #[test]
    fn test_unit_share32_mul_scalar_rounded() {
        let half = UnitShare32(500_000_000); // 50%
        let quarter = UnitShare32(250_000_000); // 25%

        // Test cases where rounding makes no difference
        assert_eq!(half.mul_scalar_rounded(100_u64), 50_u64);
        assert_eq!(quarter.mul_scalar_rounded(100_u64), 25_u64);
        assert_eq!(UnitShare32::MAX.mul_scalar_rounded(100_u64), 100_u64);
        assert_eq!(UnitShare32::MIN.mul_scalar_rounded(100_u64), 0_u64);

        // Test high precision rounding
        assert_eq!(UnitShare32(1).mul_scalar_rounded(1_000_000_000_u64), 1_u64);
        assert_eq!(UnitShare32(1).mul_scalar_rounded(500_000_000_u64), 1_u64); // 0.5 rounds up
        assert_eq!(UnitShare32(1).mul_scalar_rounded(499_999_999_u64), 0_u64); // <0.5 rounds down
    }

    #[test]
    fn test_mul_scalar_vs_mul_scalar_rounded_comparison() {
        // This test documents the difference between truncating and rounding
        let five_pct = UnitShare16(500); // 5%

        // Case 1: Exact division - both methods should give same result
        let exact_input = 542321360_u64; // 5% = 27116068 exactly
        assert_eq!(
            five_pct.mul_scalar(exact_input),
            five_pct.mul_scalar_rounded(exact_input)
        );

        // Case 2: Fractional part < 0.5 - both should truncate/round down
        let low_fraction = 542321369_u64; // 5% = 27116068.45
        assert_eq!(five_pct.mul_scalar(low_fraction), 27116068_u64); // truncates
        assert_eq!(five_pct.mul_scalar_rounded(low_fraction), 27116068_u64); // rounds down

        // Case 3: Fractional part >= 0.5 - methods differ
        let high_fraction = 542321371_u64; // 5% = 27116068.55
        assert_eq!(five_pct.mul_scalar(high_fraction), 27116068_u64); // truncates
        assert_eq!(five_pct.mul_scalar_rounded(high_fraction), 27116069_u64); // rounds up

        // Demonstrate the +1 difference
        assert_eq!(
            five_pct.mul_scalar_rounded(high_fraction) - five_pct.mul_scalar(high_fraction),
            1_u64
        );
    }

    #[test]
    fn test_mul_scalar_rounded_edge_cases() {
        // Test with MIN and zero
        assert_eq!(UnitShare16::MIN.mul_scalar_rounded(u64::MAX), 0_u64);
        assert_eq!(UnitShare16::MIN.mul_scalar_rounded(0_u64), 0_u64);

        // Test with MAX - this should return the input value
        // But note: u64::MAX with rounding will saturate due to the +denominator/2
        // So we test with smaller but still large values
        assert_eq!(UnitShare16::MAX.mul_scalar_rounded(100_u64), 100_u64);
        assert_eq!(
            UnitShare16::MAX.mul_scalar_rounded(1_000_000_u64),
            1_000_000_u64
        );

        // Test with 99% to avoid overflow issues with u64::MAX
        let ninety_nine_pct = UnitShare16(9_900); // 99%
        assert_eq!(ninety_nine_pct.mul_scalar_rounded(100_u64), 99_u64);
        assert_eq!(ninety_nine_pct.mul_scalar_rounded(1_000_u64), 990_u64);

        // Test boundary at 0.5 rounding
        let one_pct = UnitShare16(100); // 1%
        assert_eq!(one_pct.mul_scalar_rounded(49_u64), 0_u64); // 0.49 rounds down
        assert_eq!(one_pct.mul_scalar_rounded(50_u64), 1_u64); // 0.50 rounds up
        assert_eq!(one_pct.mul_scalar_rounded(51_u64), 1_u64); // 0.51 rounds up

        // Test that rounding doesn't break on large realistic values
        let five_pct = UnitShare16(500); // 5%
        let large_reward = 10_000_000_000_000_u64; // 10,000 SOL in lamports
        let result = five_pct.mul_scalar_rounded(large_reward);
        assert_eq!(result, 500_000_000_000_u64); // 500 SOL
    }

    #[test]
    fn test_validator_fee_realistic_scenarios_rounded() {
        // Test with realistic validator reward amounts
        let five_pct = UnitShare16(500); // 5% fee

        // Realistic block rewards (in lamports)
        let scenarios = vec![
            (100_000_000_u64, 5_000_000_u64),      // 0.1 SOL -> 0.005 SOL
            (1_000_000_000_u64, 50_000_000_u64),   // 1 SOL -> 0.05 SOL
            (10_000_000_000_u64, 500_000_000_u64), // 10 SOL -> 0.5 SOL
        ];

        for (reward, expected) in scenarios {
            let truncated = five_pct.mul_scalar(reward);
            let rounded = five_pct.mul_scalar_rounded(reward);

            // For these exact multiples, both should match
            assert_eq!(truncated, expected);
            assert_eq!(rounded, expected);
        }

        // Test odd amounts that would differ
        let odd_reward = 27_116_069_u64; // Creates fractional result
        let truncated = five_pct.mul_scalar(odd_reward);
        let rounded = five_pct.mul_scalar_rounded(odd_reward);

        // Document that rounded is always >= truncated
        assert!(rounded >= truncated);
        assert!(rounded - truncated <= 1); // Difference is at most 1 lamport
    }

    #[test]
    fn test_multiple_fee_components_rounded() {
        // Simulate the actual validator debt calculation with multiple fee components
        let base_fee = UnitShare16(500); // 5%
        let priority_fee = UnitShare16(500); // 5%
        let jito_fee = UnitShare16(500); // 5%
        let inflation_fee = UnitShare16(500); // 5%

        let base_rewards = 100_000_000_u64;
        let priority_rewards = 50_000_000_u64;
        let jito_rewards = 25_000_000_u64;
        let inflation_rewards = 75_000_000_u64;

        // Calculate total debt (truncated)
        let total_truncated = base_fee.mul_scalar(base_rewards)
            + priority_fee.mul_scalar(priority_rewards)
            + jito_fee.mul_scalar(jito_rewards)
            + inflation_fee.mul_scalar(inflation_rewards);

        // Calculate total debt (rounded)
        let total_rounded = base_fee.mul_scalar_rounded(base_rewards)
            + priority_fee.mul_scalar_rounded(priority_rewards)
            + jito_fee.mul_scalar_rounded(jito_rewards)
            + inflation_fee.mul_scalar_rounded(inflation_rewards);

        // With multiple components, difference can accumulate (up to 4 lamports in this case)
        assert!(total_rounded >= total_truncated);
        assert!(total_rounded - total_truncated <= 4);
    }
}
