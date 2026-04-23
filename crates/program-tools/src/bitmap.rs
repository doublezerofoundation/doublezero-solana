/// Read the bit at absolute `index` in `data`. `None` if out of range.
#[inline]
pub fn bit_at(data: &[u8], index: usize) -> Option<bool> {
    let byte_index = index / 8;
    let bit_index = index % 8;
    data.get(byte_index).map(|b| (b & (1 << bit_index)) != 0)
}

/// Set the bit at absolute `index` in `data` to `value`. `None` if out of range.
#[inline]
pub fn set_bit_at(data: &mut [u8], index: usize, value: bool) -> Option<()> {
    let byte_index = index / 8;
    let bit_index = index % 8;
    let byte = data.get_mut(byte_index)?;
    if value {
        *byte |= 1 << bit_index;
    } else {
        *byte &= !(1 << bit_index);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_at_reads_set_bits() {
        let data = [0b1001_0011u8, 0b0000_0001];

        assert_eq!(bit_at(&data, 0), Some(true));
        assert_eq!(bit_at(&data, 1), Some(true));
        assert_eq!(bit_at(&data, 2), Some(false));
        assert_eq!(bit_at(&data, 3), Some(false));
        assert_eq!(bit_at(&data, 4), Some(true));
        assert_eq!(bit_at(&data, 5), Some(false));
        assert_eq!(bit_at(&data, 6), Some(false));
        assert_eq!(bit_at(&data, 7), Some(true));
        assert_eq!(bit_at(&data, 8), Some(true));
        assert_eq!(bit_at(&data, 9), Some(false));
    }

    #[test]
    fn bit_at_out_of_range_returns_none() {
        let data = [0xFFu8; 2];
        assert_eq!(bit_at(&data, 15), Some(true));
        assert_eq!(bit_at(&data, 16), None);
        assert_eq!(bit_at(&data, usize::MAX), None);
    }

    #[test]
    fn set_bit_at_flips_individual_bits() {
        let mut data = [0u8; 2];

        for idx in [0usize, 7, 8, 15] {
            assert_eq!(set_bit_at(&mut data, idx, true), Some(()));
            assert_eq!(bit_at(&data, idx), Some(true));
        }

        assert_eq!(set_bit_at(&mut data, 7, false), Some(()));
        assert_eq!(bit_at(&data, 7), Some(false));
        assert_eq!(bit_at(&data, 0), Some(true));
        assert_eq!(bit_at(&data, 8), Some(true));
        assert_eq!(bit_at(&data, 15), Some(true));
    }

    #[test]
    fn set_bit_at_out_of_range_returns_none() {
        let mut data = [0u8; 1];
        assert_eq!(set_bit_at(&mut data, 8, true), None);
        assert_eq!(set_bit_at(&mut data, usize::MAX, true), None);
        assert_eq!(data, [0]);
    }
}
