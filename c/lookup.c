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
 
#include "lookup.h"
#include <immintrin.h>

void fast_lookup(const uint64_t** internal_pages, const uint64_t num_internal_pages,
                 const uint64_t* leaf_page,
                 const uint64_t query,
                 uint64_t* const out1, uint64_t* const out2) {
  
  // load the key
  int64_t query_as_signed = *(int64_t*)&query;
  __m512i key = _mm512_set1_epi64(query_as_signed);

  // go down the tree
  uint64_t idx = 0;
  for (size_t i = 0; i < num_internal_pages; i++) {
    __m512i page = _mm512_load_si512(internal_pages[i] + idx);
    __mmask8 res = _mm512_cmpgt_epu64_mask(key, page);
    idx = internal_pages[i][idx + 8 + __builtin_popcount(res)];
  }

  __m512i page = _mm512_load_si512(leaf_page + idx);
  __mmask8 res = _mm512_cmpge_epu64_mask(key, page);
  int gt_count = __builtin_popcount(res);

  if (gt_count == 0) {
    *out1 = leaf_page[idx - 16 + 15];
    *out2 = leaf_page[idx + 8];
  } else if (gt_count == 8) {
    *out1 = leaf_page[idx+15];
    *out2 = leaf_page[idx+16+8];
  } else {
    *out1 = leaf_page[idx + 8 + (gt_count - 1)];
    *out2 = leaf_page[idx + 8 + (gt_count - 1) + 1];
  }
}
