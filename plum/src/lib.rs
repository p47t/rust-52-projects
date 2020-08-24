extern crate bit_vec;

use bit_vec::BitVec;
use std::collections::hash_map::{DefaultHasher, RandomState};
use std::marker::PhantomData;
use std::hash::{BuildHasher, Hash, Hasher};

pub struct StandardBloomFilter<T: ?Sized> {
    bitmap: BitVec,
    optimal_m: u64,
    optimal_k: u32,
    hashers: [DefaultHasher; 2],
    _marker: PhantomData<T>,
}

fn bitmap_size(items_count: usize, fp_rate: f64) -> usize {
    let ln2_2 = core::f64::consts::LN_2 * core::f64::consts::LN_2;
    ((-1.0f64 * items_count as f64 * fp_rate.ln()) / ln2_2).ceil() as usize
}

fn optimal_k(fp_rate: f64) -> u32 {
    ((-1.0f64 * fp_rate.ln()) / core::f64::consts::LN_2).ceil() as u32
}

impl<T: ?Sized> StandardBloomFilter<T> {
    pub fn new(items_count: usize, fp_rate: f64) -> Self {
        let optimal_m = bitmap_size(items_count, fp_rate);
        let optimal_k = optimal_k(fp_rate);
        let hashers = [
            RandomState::new().build_hasher(),
            RandomState::new().build_hasher(),
        ];
        StandardBloomFilter {
            bitmap: BitVec::from_elem(optimal_m as usize, false),
            optimal_m: optimal_m as u64,
            optimal_k,
            hashers,
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, item: &T) where T: Hash {
        let (h1, h2) = self.hash_kernel(item);
        for k_i in 0..self.optimal_k {
            let index = self.get_index(h1, h2, k_i as u64);
            self.bitmap.set(index, true);
        }
    }

    fn hash_kernel(&self, item: &T) -> (u64, u64) where T: Hash {
        let hasher1 = &mut self.hashers[0].clone();
        let hasher2 = &mut self.hashers[1].clone();
        item.hash(hasher1);
        item.hash(hasher2);

        (hasher1.finish(), hasher2.finish())
    }

    fn get_index(&self, h1: u64, h2: u64, k_i: u64) -> usize {
        (h1.wrapping_add(k_i.wrapping_mul(h2)) % self.optimal_m) as usize
    }

    pub fn contains(&self, item: &T) -> bool where T: Hash {
        let (h1, h2) = self.hash_kernel(item);
        for k_i in 0..self.optimal_k {
            let index = self.get_index(h1, h2, k_i as u64);
            if !self.bitmap.get(index).unwrap() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_optimal_k_m() {
        let items_count = 1_000_000;
        let bitmap_size = bitmap_size(items_count, 0.01);
        println!("bitmap_size: {}", bitmap_size);
        let optimal_k = optimal_k(0.01);
        println!("optimal_k: {}", optimal_k);
    }

    #[test]
    fn insert() {
        let mut bf = StandardBloomFilter::new(100, 0.01);
        bf.insert("item");
        assert!(bf.contains("item"));
    }
}