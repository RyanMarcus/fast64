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
 
//pub extern "C" fn create_fast64(keys: *const u64, num_keys: u64,
//                                values: *const u64, num_values: u64)
//                                -> *const Fast64
struct Fast64;

Fast64* create_fast64(const uint64_t* keys, uint64_t num_keys,
                          const uint64_t* values, uint64_t num_values);


//pub extern "C" fn lookup_fast64(tree: *const Fast64, key: u64,
//                                out1: *mut u64, out2: *mut u64)

void lookup_fast64(Fast64* tree, uint64_t key,
                   uint64_t* out1, uint64_t* out2);

//pub extern "C" fn destroy_fast64(tree: *mut Fast64)
void destroy_fast64(Fast64* tree);
