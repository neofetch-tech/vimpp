#ifndef VIMPP_BUFFER_H
#define VIMPP_BUFFER_H
#include <stddef.h>
typedef struct VimppBuffer VimppBuffer;
VimppBuffer *vimpp_buffer_new(void);
void vimpp_buffer_free(VimppBuffer *buffer);
const char *vimpp_buffer_data(const VimppBuffer *buffer);
size_t vimpp_buffer_len(const VimppBuffer *buffer);
int vimpp_buffer_set(VimppBuffer *buffer, const char *data, size_t len);
int vimpp_buffer_insert(VimppBuffer *buffer, size_t at, const char *data, size_t len);
int vimpp_buffer_delete(VimppBuffer *buffer, size_t at, size_t len);
#endif
