// < begin copyright > 
// Copyright Ryan Marcus 2020
// 
// This file is part of fast64.
// 
// fast64 is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// 
// fast64 is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
// 
// You should have received a copy of the GNU General Public License
// along with fast64.  If not, see <http://www.gnu.org/licenses/>.
// 
// < end copyright > 
 
use std::alloc::{Layout, alloc_zeroed};
use std::mem::{drop, size_of};
use std::ptr::{copy_nonoverlapping};
use std::slice;

#[link(name="lookup")]
extern "C" {
    fn fast_lookup(
        internal_pages: *const *const u64,
        num_internal_pages: u64,
        leaf_page: *const u64,
        query: u64,
        out1: *mut u64, out2: *mut u64
    );
}
                          

fn copy_to_aligned(inp: Vec<u64>) -> Vec<u64> {
    let aligned_layout = Layout::from_size_align(inp.len() * size_of::<u64>(), 128)
        .unwrap();
    let new_vec = unsafe {
        let new_vec_mem = alloc_zeroed(aligned_layout) as *mut u64;
        copy_nonoverlapping(inp.as_ptr(), new_vec_mem, inp.len());
        Vec::from_raw_parts(new_vec_mem, inp.len(), inp.len())
    };

    return new_vec;
}

fn layout(left_vals: Vec<u64>, right_vals: Vec<u64>) -> Vec<u64> {
    return left_vals.chunks(8).zip(right_vals.chunks(8)).flat_map(|(lchk, rchk)| {
        let mut result = Vec::new();
        for i in 0..8 {
            if i >= lchk.len() {
                result.push(std::u64::MAX);
            } else {
                result.push(lchk[i]);
            }
        }

        for i in 0..8 {
            if i >= rchk.len() {
                result.push(0);
            } else {
                result.push(rchk[i]);
            }
        }

        return result;
    }).collect();
}

fn build_leaf_layer(keys: Vec<u64>, values: Vec<u64>) -> Vec<u64> {
    assert_eq!(keys.len(), values.len());
    return layout(keys, values);
}

fn build_internal_layer(prev_layer: &[u64]) -> Vec<u64> {
    assert_eq!(prev_layer.len() % 16, 0);
    let mut keys = Vec::new();
    let mut indexes = Vec::new();
    for idx in (0..prev_layer.len()).step_by(16) {
        // add the last key of each leaf page to keys
        // and the index of the leaf page to indexes
        let last_key = prev_layer[idx + 7];
        let index = idx;

        keys.push(last_key);
        indexes.push(index as u64);
    }

    // chunk keys and indexes into size 8, then flatten.
    return layout(keys, indexes);
}

fn count_greater(data: &[u64], key: u64) -> usize {
    let mut gt_count = 0;
    for &itm in data {
        if key > itm {
            gt_count += 1;
        }
    }

    return gt_count;
}

fn count_greater_eq(data: &[u64], key: u64) -> usize {
    let mut gt_count = 0;
    for &itm in data {
        if key >= itm {
            gt_count += 1;
        }
    }

    return gt_count;
}

#[repr(C)]
#[no_mangle]
pub struct Fast64 {
    leaf_layer: Vec<u64>,
    internal_layers: Vec<Vec<u64>>,
    internal_ptrs: Vec<*const u64>,
    min_key: u64, max_key: u64,
    min_val: u64, max_val: u64
}

impl Fast64 {
    pub fn new(keys: Vec<u64>, values: Vec<u64>) -> Fast64 {
        let min_key = *keys.first().unwrap();
        let max_key = *keys.last().unwrap();
        let min_val = *values.first().unwrap();
        let max_val = *values.last().unwrap();
        
        let leaf_layer = copy_to_aligned(build_leaf_layer(keys, values));
        let mut internal_layers = vec![build_internal_layer(&leaf_layer)];
        
        loop {
            let current_top_len = internal_layers.last().unwrap().len();
            if current_top_len <= 16 {
                break;
            }
            let next_layer = build_internal_layer(internal_layers.last().unwrap());
            internal_layers.push(next_layer);
        }
        internal_layers.reverse();

        internal_layers = internal_layers
            .into_iter()
            .map(|v| copy_to_aligned(v)).collect();

        let internal_ptrs: Vec<*const u64> = internal_layers.iter()
            .map(|l| l.as_ptr()).collect();
        
        return Fast64 { leaf_layer, internal_layers, internal_ptrs,
                            min_key, max_key, min_val, max_val };
    }

    pub fn fast_lookup(&self, key: u64) -> (u64, u64) {
        if key < self.min_key {
            return (0, self.min_val);
        }

        if key >= self.max_key {
            return (self.max_val, std::u64::MAX);
        }
        
        let mut v1: u64 = 0;
        let mut v2: u64 = 0;

        unsafe {
            fast_lookup(
                self.internal_ptrs.as_ptr(),
                self.internal_ptrs.len() as u64,
                self.leaf_layer.as_ptr(),
                key,
                &mut v1, &mut v2
            );
        };

        return (v1, v2);
    }
    
    pub fn slow_lookup(&self, key: u64) -> (u64, u64) {
        if key < self.min_key {
            return (0, self.min_val);
        }

        if key >= self.max_key {
            return (self.max_val, std::u64::MAX);
        }
        
        let mut idx = 0;
        for layer in self.internal_layers.iter() {
            let gt_count = count_greater(&layer[idx..idx+8], key);
            debug_assert!(gt_count < 8, "Key {} was greater than all members", key);

            idx = layer[idx+8+gt_count] as usize;
        }

        // check the leaf layer
        let gt_count = count_greater_eq(&self.leaf_layer[idx..idx+8], key);
        return if gt_count == 0 {
            // we have to go back a page to get the lower bound
            let first_idx = self.leaf_layer[idx - 16 + 15];
            let second_idx = self.leaf_layer[idx + 8];
            (first_idx, second_idx)
        } else if gt_count == 8 {
            // we have to go forward a page to get the upper bound
            let first_idx = self.leaf_layer[idx+15];
            let second_idx = self.leaf_layer[idx+16+8];
            (first_idx, second_idx)
        } else {
            let first_idx = self.leaf_layer[idx+8+(gt_count-1)];
            let second_idx = self.leaf_layer[idx+8+(gt_count-1)+1];
            (first_idx, second_idx)
        }
    }

    pub fn depth(&self) -> usize {
        return 1 + self.internal_layers.len();
    }

    pub fn size(&self) -> usize {
        let internal_size: usize = self.internal_layers
            .iter().map(|v| v.len() * size_of::<u64>())
            .sum();
        
        return (self.leaf_layer.len() * size_of::<u64>())
            + internal_size;
    }
}

// C API
#[no_mangle]
pub extern "C" fn create_fast64(keys: *const u64, num_keys: u64,
                                values: *const u64, num_values: u64)
                                -> *const Fast64 {
    let (key_vec, val_vec) = unsafe {
        let key_slice = slice::from_raw_parts(keys, num_keys as usize);
        let val_slice = slice::from_raw_parts(values, num_values as usize);
        (key_slice.to_vec(), val_slice.to_vec())
    };

    return Box::into_raw(Box::new(Fast64::new(key_vec, val_vec)));
}

#[no_mangle]
pub extern "C" fn lookup_fast64(tree: *const Fast64, key: u64,
                                out1: *mut u64, out2: *mut u64) {
    let tree_ref = unsafe { &*tree };
    let (v1, v2) = tree_ref.fast_lookup(key);

    unsafe {
        *out1 = v1;
        *out2 = v2;
    }
}

#[no_mangle]
pub extern "C" fn destroy_fast64(tree: *mut Fast64) {
    let x = unsafe { Box::from_raw(tree) };
    drop(x);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small() {
        let keys: Vec<u64> = vec![2, 4, 6, 8];
        let values: Vec<u64> = vec![10, 20, 30, 40];

        let tree = Fast64::new(keys, values);

        assert_eq!(tree.slow_lookup(0), (0, 10));
        assert_eq!(tree.slow_lookup(1), (0, 10));
        assert_eq!(tree.slow_lookup(2), (10, 20));
        assert_eq!(tree.slow_lookup(3), (10, 20));
        assert_eq!(tree.slow_lookup(4), (20, 30));
        assert_eq!(tree.slow_lookup(5), (20, 30));
        assert_eq!(tree.slow_lookup(6), (30, 40));
        assert_eq!(tree.slow_lookup(7), (30, 40));
        assert_eq!(tree.slow_lookup(8), (40, std::u64::MAX));

        assert_eq!(tree.fast_lookup(0), (0, 10));
        assert_eq!(tree.fast_lookup(1), (0, 10));
        assert_eq!(tree.fast_lookup(2), (10, 20));
        assert_eq!(tree.fast_lookup(3), (10, 20));
        assert_eq!(tree.fast_lookup(4), (20, 30));
        assert_eq!(tree.fast_lookup(5), (20, 30));
        assert_eq!(tree.fast_lookup(6), (30, 40));
        assert_eq!(tree.fast_lookup(7), (30, 40));
        assert_eq!(tree.fast_lookup(8), (40, std::u64::MAX));

    }
    
    #[test]
    fn precise_lookups() {
        let keys: Vec<u64> = (0..4096).collect();
        let values: Vec<u64> = (0..4096).collect();
        
        let tree = Fast64::new(keys, values);

        for i in 0..4095 {
            assert_eq!(tree.slow_lookup(i), (i, i+1));
            assert_eq!(tree.fast_lookup(i), (i, i+1));
        }
        assert_eq!(tree.slow_lookup(4095), (4095, std::u64::MAX));
        assert_eq!(tree.fast_lookup(4095), (4095, std::u64::MAX));
    }

    #[test]
    fn imprecise_lookups() {
        let keys: Vec<u64> = (2..8192).step_by(2).collect();
        let values: Vec<u64> = (2..8192).step_by(2).collect();
        
        let tree = Fast64::new(keys, values);

        assert_eq!(tree.slow_lookup(0), (0, 2));
        assert_eq!(tree.fast_lookup(0), (0, 2));

        for i in (3..8191).step_by(2) {
            assert_eq!(tree.slow_lookup(i), (i-1, i+1));
            assert_eq!(tree.fast_lookup(i), (i-1, i+1));
        }

        for i in (2..8190).step_by(2) {
            assert_eq!(tree.slow_lookup(i), (i, i+2));
            assert_eq!(tree.fast_lookup(i), (i, i+2));
        }
        assert_eq!(tree.slow_lookup(8190), (8190, std::u64::MAX));
        assert_eq!(tree.fast_lookup(8190), (8190, std::u64::MAX));
    }

}
