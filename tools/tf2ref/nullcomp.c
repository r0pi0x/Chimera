// Ours (Chimera project). A Wayland compositor that shows nothing: just
// enough globals (wl_compositor, wl_shm, one wl_output) for a rootless
// Xwayland to run on, so X clients can start on a box with no display.
// Buffers are released at once and frame callbacks fire at every commit.
#include <wayland-server.h>
#include <wayland-server-protocol.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static uint32_t now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000);
}

struct surf {
    struct wl_resource *buffer;
    struct wl_list frames; // pending callbacks (wl_resource links)
};

static void destroy_res(struct wl_client *c, struct wl_resource *r) { (void)c; wl_resource_destroy(r); }

// wl_region
static void region_add(struct wl_client *c, struct wl_resource *r, int32_t x, int32_t y, int32_t w, int32_t h) {
    (void)c; (void)r; (void)x; (void)y; (void)w; (void)h;
}
static const struct wl_region_interface region_impl = {destroy_res, region_add, region_add};

// wl_surface
static void surf_attach(struct wl_client *c, struct wl_resource *r, struct wl_resource *b, int32_t x, int32_t y) {
    (void)c; (void)x; (void)y;
    struct surf *s = wl_resource_get_user_data(r);
    s->buffer = b;
}
static void surf_damage(struct wl_client *c, struct wl_resource *r, int32_t x, int32_t y, int32_t w, int32_t h) {
    (void)c; (void)r; (void)x; (void)y; (void)w; (void)h;
}
static void cb_unlink(struct wl_resource *r) { wl_list_remove(wl_resource_get_link(r)); }
static void surf_frame(struct wl_client *c, struct wl_resource *r, uint32_t id) {
    struct surf *s = wl_resource_get_user_data(r);
    struct wl_resource *cb = wl_resource_create(c, &wl_callback_interface, 1, id);
    wl_resource_set_implementation(cb, NULL, NULL, cb_unlink);
    wl_list_insert(s->frames.prev, wl_resource_get_link(cb));
}
static void surf_region(struct wl_client *c, struct wl_resource *r, struct wl_resource *g) {
    (void)c; (void)r; (void)g;
}
static void surf_commit(struct wl_client *c, struct wl_resource *r) {
    (void)c;
    struct surf *s = wl_resource_get_user_data(r);
    if (s->buffer) {
        wl_buffer_send_release(s->buffer);
        s->buffer = NULL;
    }
    struct wl_resource *cb, *tmp;
    wl_resource_for_each_safe(cb, tmp, &s->frames) {
        wl_callback_send_done(cb, now_ms());
        wl_resource_destroy(cb);
    }
}
static void surf_int(struct wl_client *c, struct wl_resource *r, int32_t v) { (void)c; (void)r; (void)v; }
static void surf_offset(struct wl_client *c, struct wl_resource *r, int32_t x, int32_t y) {
    (void)c; (void)r; (void)x; (void)y;
}
static const struct wl_surface_interface surf_impl = {
    destroy_res, surf_attach, surf_damage, surf_frame, surf_region, surf_region,
    surf_commit, surf_int, surf_int, surf_damage, surf_offset};
static void surf_free(struct wl_resource *r) {
    struct surf *s = wl_resource_get_user_data(r);
    struct wl_resource *cb, *tmp;
    wl_resource_for_each_safe(cb, tmp, &s->frames) wl_resource_destroy(cb);
    free(s);
}

// wl_compositor
static void comp_surface(struct wl_client *c, struct wl_resource *r, uint32_t id) {
    struct surf *s = calloc(1, sizeof *s);
    wl_list_init(&s->frames);
    struct wl_resource *res = wl_resource_create(c, &wl_surface_interface, wl_resource_get_version(r), id);
    wl_resource_set_implementation(res, &surf_impl, s, surf_free);
}
static void comp_region(struct wl_client *c, struct wl_resource *r, uint32_t id) {
    struct wl_resource *res = wl_resource_create(c, &wl_region_interface, wl_resource_get_version(r), id);
    wl_resource_set_implementation(res, &region_impl, NULL, NULL);
}
static const struct wl_compositor_interface comp_impl = {comp_surface, comp_region};
static void bind_comp(struct wl_client *c, void *d, uint32_t v, uint32_t id) {
    (void)d;
    struct wl_resource *r = wl_resource_create(c, &wl_compositor_interface, v, id);
    wl_resource_set_implementation(r, &comp_impl, NULL, NULL);
}

// wl_output: one 1920x1080 screen
static void out_release(struct wl_client *c, struct wl_resource *r) { (void)c; wl_resource_destroy(r); }
static const struct wl_output_interface out_impl = {out_release};
static void bind_out(struct wl_client *c, void *d, uint32_t v, uint32_t id) {
    (void)d;
    struct wl_resource *r = wl_resource_create(c, &wl_output_interface, v, id);
    wl_resource_set_implementation(r, &out_impl, NULL, NULL);
    wl_output_send_geometry(r, 0, 0, 520, 290, WL_OUTPUT_SUBPIXEL_UNKNOWN, "null", "null",
                            WL_OUTPUT_TRANSFORM_NORMAL);
    wl_output_send_mode(r, WL_OUTPUT_MODE_CURRENT | WL_OUTPUT_MODE_PREFERRED, 1920, 1080, 60000);
    if (v >= 2) wl_output_send_scale(r, 1);
    if (v >= 4) wl_output_send_name(r, "NULL-1");
    if (v >= 2) wl_output_send_done(r);
}

int main(int argc, char **argv) {
    const char *name = argc > 1 ? argv[1] : "chimera-null-0";
    struct wl_display *d = wl_display_create();
    if (wl_display_add_socket(d, name)) {
        fprintf(stderr, "nullcomp: cannot add socket %s\n", name);
        return 1;
    }
    wl_display_init_shm(d);
    wl_global_create(d, &wl_compositor_interface, 5, NULL, bind_comp);
    wl_global_create(d, &wl_output_interface, 4, NULL, bind_out);
    fprintf(stderr, "nullcomp: listening on %s\n", name);
    wl_display_run(d);
    wl_display_destroy(d);
    return 0;
}
