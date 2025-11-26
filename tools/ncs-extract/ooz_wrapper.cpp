// C wrapper for oozlin's Kraken decompressor
#include "../../lib/oozlin/stdafx.h"
#include "../../lib/oozlin/kraken.h"

extern "C" {

// Wrapper function with C linkage for FFI
int ooz_kraken_decompress(
    const unsigned char* src,
    size_t src_len,
    unsigned char* dst,
    size_t dst_len
) {
    return Kraken_Decompress(src, src_len, dst, dst_len);
}

}
