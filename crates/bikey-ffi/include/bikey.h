/*
 * bikey C API: the Vietnamese input engine for native input method shims.
 *
 * Threading: a BikeyEngine is not thread-safe. Use one per input context,
 * from one thread at a time.
 *
 * Strings: all text is UTF-8 with an explicit length and is NOT
 * NUL-terminated. Text in BikeyOutput points into the engine and stays valid
 * only until the next call on the same engine; copy it if you need it longer.
 *
 * Return codes for functions returning int32_t:
 *   BIKEY_HANDLED (1)      the key was used; apply `out`
 *   BIKEY_NOT_HANDLED (0)  not a word key: commit, then pass the key on
 *   negative               error (see below); `out` is empty
 * Setters return BIKEY_OK (0) or a negative error.
 */
#ifndef BIKEY_H
#define BIKEY_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define BIKEY_OK 0
#define BIKEY_HANDLED 1
#define BIKEY_NOT_HANDLED 0
#define BIKEY_ERR_NULL (-1)    /* a required pointer was NULL */
#define BIKEY_ERR_INVALID (-2) /* an argument was out of range or not UTF-8 */
#define BIKEY_ERR_PANIC (-3)   /* internal bug; the engine was reset */

#define BIKEY_METHOD_TELEX 0u
#define BIKEY_METHOD_VNI 1u

#define BIKEY_TONE_TRADITIONAL 0u /* hòa, thủy */
#define BIKEY_TONE_MODERN 1u      /* hoà, thuỷ */

typedef struct BikeyEngine BikeyEngine;

/* Insert `commit` (if commit_len > 0), then show `preedit` as composition text. */
typedef struct BikeyOutput {
    const uint8_t *commit;
    size_t commit_len;
    const uint8_t *preedit;
    size_t preedit_len;
} BikeyOutput;

/* Returns NULL only if allocation fails. Free with bikey_free. */
BikeyEngine *bikey_new(void);
/* Accepts NULL. */
void bikey_free(BikeyEngine *engine);

int32_t bikey_set_method(BikeyEngine *engine, uint32_t method);
int32_t bikey_set_tone_style(BikeyEngine *engine, uint32_t style);
int32_t bikey_set_auto_english(BikeyEngine *engine, bool enabled);
/* ASCII letters only, 1-32 of them; returns BIKEY_ERR_INVALID otherwise. */
int32_t bikey_add_english_word(BikeyEngine *engine, const uint8_t *word, size_t len);

/* Feed one Unicode code point. */
int32_t bikey_key(BikeyEngine *engine, uint32_t codepoint, BikeyOutput *out);
/* BIKEY_NOT_HANDLED: nothing composed, pass Backspace to the application. */
int32_t bikey_backspace(BikeyEngine *engine, BikeyOutput *out);
/* Finish the word; the text to insert is in out->commit. Always HANDLED on success. */
int32_t bikey_commit(BikeyEngine *engine, BikeyOutput *out);
/* Drop the current word (e.g. Escape, focus lost in a password field). */
void bikey_reset(BikeyEngine *engine);

#ifdef __cplusplus
}
#endif

#endif /* BIKEY_H */
