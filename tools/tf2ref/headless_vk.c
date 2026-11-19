// Ours (Chimera project). A Vulkan layer that adds VK_EXT_headless_surface
// on drivers without it (NVIDIA): surfaces and swapchains backed by plain
// images, presents that only wait. Lets a game that draws through SDL's
// "offscreen" video driver and Vulkan run with no display server; its own
// screenshot command reads the frames back.
#define VK_USE_PLATFORM_HEADLESS_EXT 1
#include <vulkan/vulkan.h>
#include <vulkan/vk_layer.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>

#define EXPORT __attribute__((visibility("default")))

static PFN_vkGetInstanceProcAddr next_gipa;
static VkInstance g_instance;
static PFN_vkGetPhysicalDeviceMemoryProperties i_mem_props;
static PFN_vkGetPhysicalDeviceQueueFamilyProperties i_qfam_props;
static PFN_vkEnumerateDeviceExtensionProperties i_enum_dev_ext;
static PFN_vkGetPhysicalDeviceSurfaceSupportKHR i_surf_support;
static PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR i_surf_caps;
static PFN_vkGetPhysicalDeviceSurfaceCapabilities2KHR i_surf_caps2;
static PFN_vkGetPhysicalDeviceSurfaceFormatsKHR i_surf_formats;
static PFN_vkGetPhysicalDeviceSurfaceFormats2KHR i_surf_formats2;
static PFN_vkGetPhysicalDeviceSurfacePresentModesKHR i_surf_modes;
static PFN_vkDestroySurfaceKHR i_destroy_surface;

#define MAGIC_SURF 0x48564b5355524631ull
#define MAGIC_SWAP 0x48564b5357415031ull
struct fsurf { unsigned long long magic; };
struct fswap {
    unsigned long long magic;
    uint32_t count, next;
    VkImage images[8];
    VkDeviceMemory mem[8];
};

struct dev {
    VkDevice device;
    VkPhysicalDevice phys;
    PFN_vkGetDeviceProcAddr gdpa;
    PFN_vkSetDeviceLoaderData set_data;
    VkQueue queue;
    pthread_mutex_t lock;
    PFN_vkCreateImage create_image;
    PFN_vkDestroyImage destroy_image;
    PFN_vkGetImageMemoryRequirements mem_reqs;
    PFN_vkAllocateMemory alloc;
    PFN_vkFreeMemory free_mem;
    PFN_vkBindImageMemory bind;
    PFN_vkQueueSubmit submit;
    PFN_vkQueuePresentKHR present;
    PFN_vkCreateSwapchainKHR create_swapchain;
    PFN_vkDestroySwapchainKHR destroy_swapchain;
    PFN_vkGetSwapchainImagesKHR get_images;
    PFN_vkAcquireNextImageKHR acquire;
    PFN_vkDestroyDevice destroy_device;
};
#define MAX_DEVS 8
static struct dev devs[MAX_DEVS];
static pthread_mutex_t g_lock = PTHREAD_MUTEX_INITIALIZER;

static void *dkey(const void *h) { return *(void **)h; }
static struct dev *find_dev(const void *h) {
    void *k = dkey(h);
    for (int i = 0; i < MAX_DEVS; i++)
        if (devs[i].device && dkey(devs[i].device) == k) return &devs[i];
    return 0;
}
static int is_fsurf(VkSurfaceKHR s) {
    return s != VK_NULL_HANDLE && ((struct fsurf *)(uintptr_t)s)->magic == MAGIC_SURF;
}
static int is_fswap(VkSwapchainKHR s) {
    return s != VK_NULL_HANDLE && ((struct fswap *)(uintptr_t)s)->magic == MAGIC_SWAP;
}

// ---------------------------------------------------------------- instance

static VkResult VKAPI_CALL l_CreateInstance(const VkInstanceCreateInfo *ci,
                                            const VkAllocationCallbacks *a, VkInstance *out) {
    VkLayerInstanceCreateInfo *li = (VkLayerInstanceCreateInfo *)ci->pNext;
    while (li && !(li->sType == VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO &&
                   li->function == VK_LAYER_LINK_INFO))
        li = (VkLayerInstanceCreateInfo *)li->pNext;
    if (!li) return VK_ERROR_INITIALIZATION_FAILED;
    PFN_vkGetInstanceProcAddr gipa = li->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    li->u.pLayerInfo = li->u.pLayerInfo->pNext;
    // Hide our extension from the layers and driver below.
    VkInstanceCreateInfo c = *ci;
    const char **names = calloc(ci->enabledExtensionCount + 1, sizeof(char *));
    uint32_t n = 0;
    for (uint32_t i = 0; i < ci->enabledExtensionCount; i++)
        if (strcmp(ci->ppEnabledExtensionNames[i], VK_EXT_HEADLESS_SURFACE_EXTENSION_NAME))
            names[n++] = ci->ppEnabledExtensionNames[i];
    c.enabledExtensionCount = n;
    c.ppEnabledExtensionNames = names;
    PFN_vkCreateInstance create = (PFN_vkCreateInstance)gipa(VK_NULL_HANDLE, "vkCreateInstance");
    VkResult r = create(&c, a, out);
    free(names);
    if (r != VK_SUCCESS) return r;
    next_gipa = gipa;
    g_instance = *out;
#define I(v, n) v = (void *)gipa(*out, n)
    I(i_mem_props, "vkGetPhysicalDeviceMemoryProperties");
    I(i_qfam_props, "vkGetPhysicalDeviceQueueFamilyProperties");
    I(i_enum_dev_ext, "vkEnumerateDeviceExtensionProperties");
    I(i_surf_support, "vkGetPhysicalDeviceSurfaceSupportKHR");
    I(i_surf_caps, "vkGetPhysicalDeviceSurfaceCapabilitiesKHR");
    I(i_surf_caps2, "vkGetPhysicalDeviceSurfaceCapabilities2KHR");
    I(i_surf_formats, "vkGetPhysicalDeviceSurfaceFormatsKHR");
    I(i_surf_formats2, "vkGetPhysicalDeviceSurfaceFormats2KHR");
    I(i_surf_modes, "vkGetPhysicalDeviceSurfacePresentModesKHR");
    I(i_destroy_surface, "vkDestroySurfaceKHR");
#undef I
    return VK_SUCCESS;
}

static VkResult VKAPI_CALL l_CreateHeadlessSurfaceEXT(VkInstance inst,
                                                      const VkHeadlessSurfaceCreateInfoEXT *ci,
                                                      const VkAllocationCallbacks *a, VkSurfaceKHR *out) {
    (void)inst; (void)ci; (void)a;
    struct fsurf *s = calloc(1, sizeof *s);
    s->magic = MAGIC_SURF;
    *out = (VkSurfaceKHR)(uintptr_t)s;
    return VK_SUCCESS;
}

static void VKAPI_CALL l_DestroySurfaceKHR(VkInstance inst, VkSurfaceKHR s, const VkAllocationCallbacks *a) {
    if (is_fsurf(s)) { ((struct fsurf *)(uintptr_t)s)->magic = 0; free((void *)(uintptr_t)s); return; }
    if (i_destroy_surface) i_destroy_surface(inst, s, a);
}

static VkResult VKAPI_CALL l_SurfaceSupport(VkPhysicalDevice p, uint32_t q, VkSurfaceKHR s, VkBool32 *out) {
    if (is_fsurf(s)) { *out = VK_TRUE; return VK_SUCCESS; }
    return i_surf_support(p, q, s, out);
}

static void fill_caps(VkSurfaceCapabilitiesKHR *c) {
    memset(c, 0, sizeof *c);
    c->minImageCount = 2;
    c->maxImageCount = 8;
    c->currentExtent.width = 0xFFFFFFFF;
    c->currentExtent.height = 0xFFFFFFFF;
    c->minImageExtent.width = 1;
    c->minImageExtent.height = 1;
    c->maxImageExtent.width = 16384;
    c->maxImageExtent.height = 16384;
    c->maxImageArrayLayers = 1;
    c->supportedTransforms = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR;
    c->currentTransform = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR;
    c->supportedCompositeAlpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
    c->supportedUsageFlags = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT |
                             VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT |
                             VK_IMAGE_USAGE_STORAGE_BIT;
}

static VkResult VKAPI_CALL l_SurfaceCaps(VkPhysicalDevice p, VkSurfaceKHR s, VkSurfaceCapabilitiesKHR *c) {
    if (is_fsurf(s)) { fill_caps(c); return VK_SUCCESS; }
    return i_surf_caps(p, s, c);
}

static VkResult VKAPI_CALL l_SurfaceCaps2(VkPhysicalDevice p, const VkPhysicalDeviceSurfaceInfo2KHR *info,
                                          VkSurfaceCapabilities2KHR *c) {
    if (is_fsurf(info->surface)) { fill_caps(&c->surfaceCapabilities); return VK_SUCCESS; }
    return i_surf_caps2(p, info, c);
}

static const VkFormat FORMATS[] = {VK_FORMAT_B8G8R8A8_UNORM, VK_FORMAT_B8G8R8A8_SRGB,
                                   VK_FORMAT_R8G8B8A8_UNORM, VK_FORMAT_R8G8B8A8_SRGB};
#define NFORMATS 4

static VkResult VKAPI_CALL l_SurfaceFormats(VkPhysicalDevice p, VkSurfaceKHR s, uint32_t *n, VkSurfaceFormatKHR *f) {
    if (!is_fsurf(s)) return i_surf_formats(p, s, n, f);
    if (!f) { *n = NFORMATS; return VK_SUCCESS; }
    uint32_t k = *n < NFORMATS ? *n : NFORMATS;
    for (uint32_t i = 0; i < k; i++) {
        f[i].format = FORMATS[i];
        f[i].colorSpace = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;
    }
    *n = k;
    return k < NFORMATS ? VK_INCOMPLETE : VK_SUCCESS;
}

static VkResult VKAPI_CALL l_SurfaceFormats2(VkPhysicalDevice p, const VkPhysicalDeviceSurfaceInfo2KHR *info,
                                             uint32_t *n, VkSurfaceFormat2KHR *f) {
    if (!is_fsurf(info->surface)) return i_surf_formats2(p, info, n, f);
    if (!f) { *n = NFORMATS; return VK_SUCCESS; }
    uint32_t k = *n < NFORMATS ? *n : NFORMATS;
    for (uint32_t i = 0; i < k; i++) {
        f[i].surfaceFormat.format = FORMATS[i];
        f[i].surfaceFormat.colorSpace = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;
    }
    *n = k;
    return k < NFORMATS ? VK_INCOMPLETE : VK_SUCCESS;
}

static VkResult VKAPI_CALL l_SurfaceModes(VkPhysicalDevice p, VkSurfaceKHR s, uint32_t *n, VkPresentModeKHR *m) {
    if (!is_fsurf(s)) return i_surf_modes(p, s, n, m);
    static const VkPresentModeKHR MODES[] = {VK_PRESENT_MODE_FIFO_KHR, VK_PRESENT_MODE_IMMEDIATE_KHR,
                                             VK_PRESENT_MODE_MAILBOX_KHR};
    if (!m) { *n = 3; return VK_SUCCESS; }
    uint32_t k = *n < 3 ? *n : 3;
    memcpy(m, MODES, k * sizeof *m);
    *n = k;
    return k < 3 ? VK_INCOMPLETE : VK_SUCCESS;
}

// Device extensions that would need a real presentation engine.
static int hidden_ext(const char *e) {
    static const char *H[] = {"VK_KHR_present_wait", "VK_KHR_present_id", "VK_KHR_present_wait2",
                              "VK_KHR_present_id2", "VK_EXT_swapchain_maintenance1",
                              "VK_KHR_swapchain_maintenance1", "VK_EXT_full_screen_exclusive",
                              "VK_EXT_hdr_metadata", "VK_KHR_swapchain_mutable_format",
                              "VK_EXT_present_timing", "VK_GOOGLE_display_timing", 0};
    for (int i = 0; H[i]; i++)
        if (!strcmp(e, H[i])) return 1;
    return 0;
}

static VkResult VKAPI_CALL l_EnumDevExt(VkPhysicalDevice p, const char *layer, uint32_t *n,
                                        VkExtensionProperties *props) {
    if (layer && !strcmp(layer, "VK_LAYER_CHIMERA_headless")) { *n = 0; return VK_SUCCESS; }
    uint32_t all = 0;
    VkResult r = i_enum_dev_ext(p, layer, &all, 0);
    if (r != VK_SUCCESS) return r;
    VkExtensionProperties *tmp = calloc(all ? all : 1, sizeof *tmp);
    r = i_enum_dev_ext(p, layer, &all, tmp);
    uint32_t k = 0;
    for (uint32_t i = 0; i < all; i++)
        if (!hidden_ext(tmp[i].extensionName)) tmp[k++] = tmp[i];
    if (!props) { *n = k; free(tmp); return VK_SUCCESS; }
    uint32_t c = *n < k ? *n : k;
    memcpy(props, tmp, c * sizeof *props);
    *n = c;
    free(tmp);
    return c < k ? VK_INCOMPLETE : VK_SUCCESS;
}

// ------------------------------------------------------------------ device

static VkResult VKAPI_CALL l_CreateDevice(VkPhysicalDevice phys, const VkDeviceCreateInfo *ci,
                                          const VkAllocationCallbacks *a, VkDevice *out) {
    VkLayerDeviceCreateInfo *li = (VkLayerDeviceCreateInfo *)ci->pNext, *cb = 0;
    for (VkLayerDeviceCreateInfo *x = li; x; x = (VkLayerDeviceCreateInfo *)x->pNext)
        if (x->sType == VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO && x->function == VK_LOADER_DATA_CALLBACK)
            cb = x;
    while (li && !(li->sType == VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO && li->function == VK_LAYER_LINK_INFO))
        li = (VkLayerDeviceCreateInfo *)li->pNext;
    if (!li) return VK_ERROR_INITIALIZATION_FAILED;
    PFN_vkGetInstanceProcAddr gipa = li->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    PFN_vkGetDeviceProcAddr gdpa = li->u.pLayerInfo->pfnNextGetDeviceProcAddr;
    li->u.pLayerInfo = li->u.pLayerInfo->pNext;

    // One more queue in family 0 for our own empty submits.
    uint32_t nf = 0;
    i_qfam_props(phys, &nf, 0);
    VkQueueFamilyProperties *fp = calloc(nf ? nf : 1, sizeof *fp);
    i_qfam_props(phys, &nf, fp);
    VkDeviceCreateInfo c = *ci;
    VkDeviceQueueCreateInfo *qs = calloc(ci->queueCreateInfoCount + 1, sizeof *qs);
    memcpy(qs, ci->pQueueCreateInfos, ci->queueCreateInfoCount * sizeof *qs);
    float *prio = 0;
    int extra = -1;
    for (uint32_t i = 0; i < ci->queueCreateInfoCount; i++) {
        if (qs[i].queueFamilyIndex == 0 && qs[i].flags == 0 && nf && qs[i].queueCount < fp[0].queueCount) {
            prio = calloc(qs[i].queueCount + 1, sizeof(float));
            memcpy(prio, qs[i].pQueuePriorities, qs[i].queueCount * sizeof(float));
            prio[qs[i].queueCount] = 0.5f;
            extra = (int)qs[i].queueCount;
            qs[i].queueCount++;
            qs[i].pQueuePriorities = prio;
        }
    }
    c.pQueueCreateInfos = qs;
    PFN_vkCreateDevice create = (PFN_vkCreateDevice)gipa(g_instance, "vkCreateDevice");
    VkResult r = create(phys, &c, a, out);
    free(qs); free(prio); free(fp);
    if (r != VK_SUCCESS) return r;

    pthread_mutex_lock(&g_lock);
    struct dev *d = 0;
    for (int i = 0; i < MAX_DEVS; i++)
        if (!devs[i].device) { d = &devs[i]; break; }
    if (d) {
        memset(d, 0, sizeof *d);
        d->device = *out;
        d->phys = phys;
        d->gdpa = gdpa;
        d->set_data = cb ? cb->u.pfnSetDeviceLoaderData : 0;
        pthread_mutex_init(&d->lock, 0);
#define D(v, n) d->v = (void *)gdpa(*out, n)
        D(create_image, "vkCreateImage");
        D(destroy_image, "vkDestroyImage");
        D(mem_reqs, "vkGetImageMemoryRequirements");
        D(alloc, "vkAllocateMemory");
        D(free_mem, "vkFreeMemory");
        D(bind, "vkBindImageMemory");
        D(submit, "vkQueueSubmit");
        D(present, "vkQueuePresentKHR");
        D(create_swapchain, "vkCreateSwapchainKHR");
        D(destroy_swapchain, "vkDestroySwapchainKHR");
        D(get_images, "vkGetSwapchainImagesKHR");
        D(acquire, "vkAcquireNextImageKHR");
        D(destroy_device, "vkDestroyDevice");
#undef D
        PFN_vkGetDeviceQueue gq = (PFN_vkGetDeviceQueue)gdpa(*out, "vkGetDeviceQueue");
        if (extra >= 0) {
            gq(*out, 0, (uint32_t)extra, &d->queue);
            if (d->set_data) d->set_data(*out, d->queue);
        } else {
            gq(*out, 0, 0, &d->queue); // shared with the app: racy, best effort
        }
    }
    pthread_mutex_unlock(&g_lock);
    return VK_SUCCESS;
}

static void VKAPI_CALL l_DestroyDevice(VkDevice dev, const VkAllocationCallbacks *a) {
    struct dev *d = find_dev(dev);
    if (!d) return;
    PFN_vkDestroyDevice f = d->destroy_device;
    pthread_mutex_lock(&g_lock);
    d->device = 0;
    pthread_mutex_unlock(&g_lock);
    f(dev, a);
}

static uint32_t mem_type(VkPhysicalDevice p, uint32_t bits) {
    VkPhysicalDeviceMemoryProperties mp;
    i_mem_props(p, &mp);
    for (uint32_t i = 0; i < mp.memoryTypeCount; i++)
        if ((bits & (1u << i)) && (mp.memoryTypes[i].propertyFlags & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT))
            return i;
    for (uint32_t i = 0; i < mp.memoryTypeCount; i++)
        if (bits & (1u << i)) return i;
    return 0;
}

static VkResult VKAPI_CALL l_CreateSwapchainKHR(VkDevice dev, const VkSwapchainCreateInfoKHR *ci,
                                                const VkAllocationCallbacks *a, VkSwapchainKHR *out) {
    struct dev *d = find_dev(dev);
    if (!is_fsurf(ci->surface)) return d->create_swapchain(dev, ci, a, out);
    struct fswap *s = calloc(1, sizeof *s);
    s->magic = MAGIC_SWAP;
    s->count = ci->minImageCount < 2 ? 2 : (ci->minImageCount > 8 ? 8 : ci->minImageCount);
    for (uint32_t i = 0; i < s->count; i++) {
        VkImageCreateInfo ic = {VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO};
        ic.imageType = VK_IMAGE_TYPE_2D;
        ic.format = ci->imageFormat;
        ic.extent.width = ci->imageExtent.width;
        ic.extent.height = ci->imageExtent.height;
        ic.extent.depth = 1;
        ic.mipLevels = 1;
        ic.arrayLayers = ci->imageArrayLayers ? ci->imageArrayLayers : 1;
        ic.samples = VK_SAMPLE_COUNT_1_BIT;
        ic.tiling = VK_IMAGE_TILING_OPTIMAL;
        ic.usage = ci->imageUsage | VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
        ic.sharingMode = ci->imageSharingMode;
        ic.queueFamilyIndexCount = ci->queueFamilyIndexCount;
        ic.pQueueFamilyIndices = ci->pQueueFamilyIndices;
        ic.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        VkResult r = d->create_image(dev, &ic, a, &s->images[i]);
        if (r != VK_SUCCESS) return r;
        VkMemoryRequirements mr;
        d->mem_reqs(dev, s->images[i], &mr);
        VkMemoryAllocateInfo ai = {VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO};
        ai.allocationSize = mr.size;
        ai.memoryTypeIndex = mem_type(d->phys, mr.memoryTypeBits);
        r = d->alloc(dev, &ai, a, &s->mem[i]);
        if (r != VK_SUCCESS) return r;
        d->bind(dev, s->images[i], s->mem[i], 0);
    }
    *out = (VkSwapchainKHR)(uintptr_t)s;
    return VK_SUCCESS;
}

static void VKAPI_CALL l_DestroySwapchainKHR(VkDevice dev, VkSwapchainKHR sw, const VkAllocationCallbacks *a) {
    struct dev *d = find_dev(dev);
    if (!is_fswap(sw)) { if (sw) d->destroy_swapchain(dev, sw, a); return; }
    struct fswap *s = (struct fswap *)(uintptr_t)sw;
    for (uint32_t i = 0; i < s->count; i++) {
        d->destroy_image(dev, s->images[i], a);
        d->free_mem(dev, s->mem[i], a);
    }
    s->magic = 0;
    free(s);
}

static VkResult VKAPI_CALL l_GetSwapchainImagesKHR(VkDevice dev, VkSwapchainKHR sw, uint32_t *n, VkImage *imgs) {
    struct dev *d = find_dev(dev);
    if (!is_fswap(sw)) return d->get_images(dev, sw, n, imgs);
    struct fswap *s = (struct fswap *)(uintptr_t)sw;
    if (!imgs) { *n = s->count; return VK_SUCCESS; }
    uint32_t k = *n < s->count ? *n : s->count;
    memcpy(imgs, s->images, k * sizeof *imgs);
    *n = k;
    return k < s->count ? VK_INCOMPLETE : VK_SUCCESS;
}

static VkResult signal(struct dev *d, uint32_t nwait, const VkSemaphore *wait, VkSemaphore sem, VkFence fence) {
    if (!nwait && !sem && !fence) return VK_SUCCESS;
    VkPipelineStageFlags stages[16];
    for (int i = 0; i < 16; i++) stages[i] = VK_PIPELINE_STAGE_ALL_COMMANDS_BIT;
    VkSubmitInfo si = {VK_STRUCTURE_TYPE_SUBMIT_INFO};
    si.waitSemaphoreCount = nwait > 16 ? 16 : nwait;
    si.pWaitSemaphores = wait;
    si.pWaitDstStageMask = stages;
    si.signalSemaphoreCount = sem ? 1 : 0;
    si.pSignalSemaphores = &sem;
    pthread_mutex_lock(&d->lock);
    VkResult r = d->submit(d->queue, 1, &si, fence);
    pthread_mutex_unlock(&d->lock);
    return r;
}

static VkResult VKAPI_CALL l_AcquireNextImageKHR(VkDevice dev, VkSwapchainKHR sw, uint64_t t, VkSemaphore sem,
                                                 VkFence fence, uint32_t *idx) {
    struct dev *d = find_dev(dev);
    if (!is_fswap(sw)) return d->acquire(dev, sw, t, sem, fence, idx);
    struct fswap *s = (struct fswap *)(uintptr_t)sw;
    *idx = s->next;
    s->next = (s->next + 1) % s->count;
    return signal(d, 0, 0, sem, fence);
}

static VkResult VKAPI_CALL l_AcquireNextImage2KHR(VkDevice dev, const VkAcquireNextImageInfoKHR *ai, uint32_t *idx) {
    return l_AcquireNextImageKHR(dev, ai->swapchain, ai->timeout, ai->semaphore, ai->fence, idx);
}

static VkResult VKAPI_CALL l_QueuePresentKHR(VkQueue q, const VkPresentInfoKHR *pi) {
    struct dev *d = find_dev(q);
    int fake = 0;
    for (uint32_t i = 0; i < pi->swapchainCount; i++)
        if (is_fswap(pi->pSwapchains[i])) fake = 1;
    if (!fake) return d->present(q, pi);
    for (uint32_t i = 0; i < pi->swapchainCount; i++)
        if (pi->pResults) pi->pResults[i] = VK_SUCCESS;
    return signal(d, pi->waitSemaphoreCount, pi->pWaitSemaphores, VK_NULL_HANDLE, VK_NULL_HANDLE);
}

static VkResult VKAPI_CALL l_GetSwapchainStatusKHR(VkDevice dev, VkSwapchainKHR sw) {
    (void)dev; (void)sw;
    return VK_SUCCESS;
}

// ---------------------------------------------------------------- dispatch

static PFN_vkVoidFunction dev_fn(const char *n) {
#define F(name, fn) if (!strcmp(n, name)) return (PFN_vkVoidFunction)fn
    F("vkCreateSwapchainKHR", l_CreateSwapchainKHR);
    F("vkDestroySwapchainKHR", l_DestroySwapchainKHR);
    F("vkGetSwapchainImagesKHR", l_GetSwapchainImagesKHR);
    F("vkAcquireNextImageKHR", l_AcquireNextImageKHR);
    F("vkAcquireNextImage2KHR", l_AcquireNextImage2KHR);
    F("vkQueuePresentKHR", l_QueuePresentKHR);
    F("vkGetSwapchainStatusKHR", l_GetSwapchainStatusKHR);
    F("vkDestroyDevice", l_DestroyDevice);
#undef F
    return 0;
}

static PFN_vkVoidFunction VKAPI_CALL l_GetDeviceProcAddr(VkDevice dev, const char *n) {
    PFN_vkVoidFunction f = dev_fn(n);
    if (f) return f;
    if (!strcmp(n, "vkGetDeviceProcAddr")) return (PFN_vkVoidFunction)l_GetDeviceProcAddr;
    struct dev *d = find_dev(dev);
    return d ? d->gdpa(dev, n) : 0;
}

static PFN_vkVoidFunction VKAPI_CALL l_GetInstanceProcAddr(VkInstance inst, const char *n) {
#define F(name, fn) if (!strcmp(n, name)) return (PFN_vkVoidFunction)fn
    F("vkGetInstanceProcAddr", l_GetInstanceProcAddr);
    F("vkGetDeviceProcAddr", l_GetDeviceProcAddr);
    F("vkCreateInstance", l_CreateInstance);
    F("vkCreateDevice", l_CreateDevice);
    F("vkCreateHeadlessSurfaceEXT", l_CreateHeadlessSurfaceEXT);
    F("vkDestroySurfaceKHR", l_DestroySurfaceKHR);
    F("vkGetPhysicalDeviceSurfaceSupportKHR", l_SurfaceSupport);
    F("vkGetPhysicalDeviceSurfaceCapabilitiesKHR", l_SurfaceCaps);
    F("vkGetPhysicalDeviceSurfaceCapabilities2KHR", l_SurfaceCaps2);
    F("vkGetPhysicalDeviceSurfaceFormatsKHR", l_SurfaceFormats);
    F("vkGetPhysicalDeviceSurfaceFormats2KHR", l_SurfaceFormats2);
    F("vkGetPhysicalDeviceSurfacePresentModesKHR", l_SurfaceModes);
    F("vkEnumerateDeviceExtensionProperties", l_EnumDevExt);
#undef F
    PFN_vkVoidFunction f = dev_fn(n);
    if (f) return f;
    return next_gipa ? next_gipa(inst, n) : 0;
}

EXPORT VkResult VKAPI_CALL vkNegotiateLoaderLayerInterfaceVersion(VkNegotiateLayerInterface *v) {
    if (v->loaderLayerInterfaceVersion > 2) v->loaderLayerInterfaceVersion = 2;
    v->pfnGetInstanceProcAddr = l_GetInstanceProcAddr;
    v->pfnGetDeviceProcAddr = l_GetDeviceProcAddr;
    v->pfnGetPhysicalDeviceProcAddr = 0;
    return VK_SUCCESS;
}
