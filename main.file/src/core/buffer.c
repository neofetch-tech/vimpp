#include "buffer.h"
#include <stdlib.h>
#include <string.h>
struct VimppBuffer { char *data; size_t len; size_t cap; };
static int reserve(VimppBuffer *b, size_t need) {
    if (need <= b->cap) return 1;
    size_t cap = b->cap ? b->cap : 64;
    while (cap < need) { if (cap > (size_t)-1 / 2) return 0; cap *= 2; }
    char *p = realloc(b->data, cap);
    if (!p) return 0;
    b->data = p; b->cap = cap; return 1;
}
VimppBuffer *vimpp_buffer_new(void) {
    VimppBuffer *b = calloc(1, sizeof(*b));
    if (b && reserve(b, 1)) b->data[0] = '\0';
    else { free(b); return NULL; }
    return b;
}
void vimpp_buffer_free(VimppBuffer *b) { if (b) { free(b->data); free(b); } }
const char *vimpp_buffer_data(const VimppBuffer *b) { return b ? b->data : ""; }
size_t vimpp_buffer_len(const VimppBuffer *b) { return b ? b->len : 0; }
int vimpp_buffer_set(VimppBuffer *b, const char *data, size_t len) {
    if (!b || !reserve(b, len + 1)) return 0;
    if (len) memcpy(b->data, data, len);
    b->len = len; b->data[len] = '\0'; return 1;
}
int vimpp_buffer_insert(VimppBuffer *b, size_t at, const char *data, size_t len) {
    if (!b || at > b->len || len > (size_t)-1 - b->len - 1 || !reserve(b, b->len + len + 1)) return 0;
    memmove(b->data + at + len, b->data + at, b->len - at + 1);
    if (len) memcpy(b->data + at, data, len);
    b->len += len; return 1;
}
int vimpp_buffer_delete(VimppBuffer *b, size_t at, size_t len) {
    if (!b || at > b->len || len > b->len - at) return 0;
    memmove(b->data + at, b->data + at + len, b->len - at - len + 1);
    b->len -= len; return 1;
}
