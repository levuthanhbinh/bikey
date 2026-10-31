/* Compiles against include/bikey.h and links the static library, so a
 * mismatch between the header and the Rust exports fails the build. */
#include <stdio.h>
#include <string.h>

#include "bikey.h"

static int expect(const char *keys, const char *want, BikeyEngine *e) {
    BikeyOutput out;
    for (const char *k = keys; *k; k++) {
        if (bikey_key(e, (uint32_t)(unsigned char)*k, &out) != BIKEY_HANDLED) {
            fprintf(stderr, "key '%c' not handled in %s\n", *k, keys);
            return 1;
        }
    }
    if (bikey_commit(e, &out) != BIKEY_HANDLED || out.commit_len != strlen(want) ||
        memcmp(out.commit, want, out.commit_len) != 0) {
        fprintf(stderr, "%s: got '%.*s', want '%s'\n", keys, (int)out.commit_len,
                (const char *)out.commit, want);
        return 1;
    }
    return 0;
}

int main(void) {
    BikeyEngine *e = bikey_new();
    if (!e) return 1;
    int failed = 0;
    failed |= expect("tieengs", "ti\xe1\xba\xbfng", e); /* tiếng */
    failed |= expect("class", "class", e);
    failed |= bikey_set_method(e, BIKEY_METHOD_VNI) != BIKEY_OK;
    failed |= expect("Vie65t", "Vi\xe1\xbb\x87t", e); /* Việt */
    failed |= bikey_key(NULL, 'a', NULL) != BIKEY_ERR_NULL;
    bikey_free(e);
    puts(failed ? "FAIL" : "ok");
    return failed;
}
