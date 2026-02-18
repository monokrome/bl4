/*
 * Oodle decompression helper for bl4 ncs --oodle-exec
 *
 * Links against liboo2corelinux64.a to provide native Oodle decompression.
 *
 * Usage:
 *   oodle_helper decompress <decompressed_size>
 *     Reads compressed data from stdin, writes decompressed data to stdout.
 *
 *   oodle_helper decompress <decompressed_size> <input_path> <output_path>
 *     Reads compressed data from input_path, writes decompressed data to output_path.
 *     Use this mode with --oodle-fifo for FIFO/named-pipe based transfer (Wine).
 *
 *   Exit code 0 on success, non-zero on error.
 *
 * Build:
 *   gcc -o oodle_helper oodle_helper.c -L. -l:liboo2corelinux64.a -lstdc++ -lm -lpthread
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/* Oodle function signature (from SDK) */
extern int64_t OodleLZ_Decompress(
    const void* compBuf,
    int64_t compLen,
    void* rawBuf,
    int64_t rawLen,
    int fuzzSafe,
    int checkCRC,
    int verbosity,
    void* decBufBase,
    int64_t decBufSize,
    void* fpCallback,
    void* callbackUserData,
    void* decoderMemory,
    int64_t decoderMemorySize,
    int threadPhase
);

#define MAX_COMPRESSED_SIZE (64 * 1024 * 1024)  /* 64 MB max */

static size_t read_all(FILE* f, uint8_t* buf, size_t max_len) {
    size_t total = 0;
    size_t n;
    while ((n = fread(buf + total, 1, max_len - total, f)) > 0) {
        total += n;
    }
    return total;
}

int main(int argc, char* argv[]) {
    int use_fifo = (argc == 5);

    if (argc != 3 && argc != 5) {
        fprintf(stderr,
            "Usage: %s decompress <size>\n"
            "       %s decompress <size> <input> <output>\n",
            argv[0], argv[0]);
        return 1;
    }

    if (strcmp(argv[1], "decompress") != 0) {
        fprintf(stderr, "Unknown command: %s\n", argv[1]);
        return 1;
    }

    int64_t decompressed_size = atoll(argv[2]);
    if (decompressed_size <= 0 || decompressed_size > MAX_COMPRESSED_SIZE) {
        fprintf(stderr, "Invalid decompressed size: %ld\n", decompressed_size);
        return 1;
    }

    /* Open input source */
    FILE* input;
    if (use_fifo) {
        input = fopen(argv[3], "rb");
        if (!input) {
            fprintf(stderr, "Failed to open input: %s\n", argv[3]);
            return 1;
        }
    } else {
        input = stdin;
    }

    /* Read compressed data */
    uint8_t* compressed = malloc(MAX_COMPRESSED_SIZE);
    if (!compressed) {
        fprintf(stderr, "Failed to allocate compressed buffer\n");
        if (use_fifo) fclose(input);
        return 1;
    }

    size_t compressed_len = read_all(input, compressed, MAX_COMPRESSED_SIZE);
    if (use_fifo) fclose(input);

    if (compressed_len == 0) {
        fprintf(stderr, "No input data\n");
        free(compressed);
        return 1;
    }

    /* Allocate output buffer */
    uint8_t* decompressed = malloc(decompressed_size);
    if (!decompressed) {
        fprintf(stderr, "Failed to allocate decompressed buffer\n");
        free(compressed);
        return 1;
    }

    /* Decompress using Oodle */
    int64_t result = OodleLZ_Decompress(
        compressed,
        compressed_len,
        decompressed,
        decompressed_size,
        1,      /* fuzzSafe */
        0,      /* checkCRC */
        0,      /* verbosity */
        NULL,   /* decBufBase */
        0,      /* decBufSize */
        NULL,   /* fpCallback */
        NULL,   /* callbackUserData */
        NULL,   /* decoderMemory */
        0,      /* decoderMemorySize */
        0       /* threadPhase */
    );

    free(compressed);

    if (result < 0) {
        fprintf(stderr, "Oodle decompression failed with code %ld\n", result);
        free(decompressed);
        return 1;
    }

    if (result != decompressed_size) {
        fprintf(stderr, "Size mismatch: expected %ld, got %ld\n",
                decompressed_size, result);
        free(decompressed);
        return 1;
    }

    /* Write output */
    FILE* output;
    if (use_fifo) {
        output = fopen(argv[4], "wb");
        if (!output) {
            fprintf(stderr, "Failed to open output: %s\n", argv[4]);
            free(decompressed);
            return 1;
        }
    } else {
        output = stdout;
    }

    size_t written = fwrite(decompressed, 1, decompressed_size, output);
    if (use_fifo) fclose(output);
    free(decompressed);

    if (written != (size_t)decompressed_size) {
        fprintf(stderr, "Failed to write output\n");
        return 1;
    }

    return 0;
}
