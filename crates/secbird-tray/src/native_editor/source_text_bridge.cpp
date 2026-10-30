// Natural DirectWrite bridge for three measured native source text roles.
// Uses the Windows SDK's COM declarations; no handwritten DirectWrite vtables.
#include <windows.h>
#include <dwrite_3.h>
#include <bcrypt.h>
#include <wrl/client.h>
#include <cmath>
#include <new>
#include <cstring>
#include <cstdint>
#include <utility>

using Microsoft::WRL::ComPtr;

namespace {
enum Status : unsigned {
    Ok, InvalidArgument, UnsupportedFactory, InvalidFont, LayoutFailure,
    BitmapFailure, RenderFailure, BlitFailure, AllocationFailure, MissingGlyph
};
constexpr unsigned MaxText = 256;
constexpr unsigned MaxLayouts = 32;
constexpr unsigned MaxWidth = 2048;
constexpr unsigned MaxHeight = 256;
#ifdef SOURCE_TEXT_PILOT_TEST
struct CleanupObservation {
    unsigned unregister_calls = 0;
    unsigned unregister_failures = 0;
    unsigned font_objects_at_unregister = 0;
};
CleanupObservation cleanup_observation;
#endif
struct Metrics {
    float advance_css;
    float layout_baseline_css;
    float source_baseline_physical;
    unsigned cached_layouts;
};
static_assert(sizeof(wchar_t) == 2 && sizeof(unsigned) == 4 && sizeof(Metrics) == 16,
    "The Rust bridge requires Windows UTF-16/u32 and a16-byte metric record");
struct Entry {
    unsigned role = 0;
    unsigned length = 0;
    uint64_t age = 0;
    wchar_t text[MaxText]{};
    ComPtr<IDWriteTextLayout> layout;
    float advance = 0;
    float baseline = 0;
};

// Read and hash a bounded local buffer. DirectWrite subsequently copies these
// same bytes with its official in-memory loader; it never reopens this path.
struct FontRead {
    HANDLE handle = INVALID_HANDLE_VALUE;
    BYTE* bytes = nullptr;
    unsigned size = 0;
    ~FontRead() {
        if (bytes) HeapFree(GetProcessHeap(), 0, bytes);
        if (handle != INVALID_HANDLE_VALUE) CloseHandle(handle);
    }
    HRESULT authenticate(const wchar_t* file, unsigned role) {
        handle = CreateFileW(file, GENERIC_READ, FILE_SHARE_READ, nullptr,
            OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN, nullptr);
        if (handle == INVALID_HANDLE_VALUE) return HRESULT_FROM_WIN32(GetLastError());
        LARGE_INTEGER file_size{};
        if (!GetFileSizeEx(handle, &file_size)) return HRESULT_FROM_WIN32(GetLastError());
        const unsigned expected_size = role == 0 ? 72204 : role == 1 ? 35408 : 35468;
        if (file_size.QuadPart != expected_size) return DWRITE_E_FILEFORMAT;
        size = expected_size;
        bytes = static_cast<BYTE*>(HeapAlloc(GetProcessHeap(), 0, size));
        if (!bytes) return E_OUTOFMEMORY;
        DWORD read = 0;
        if (!ReadFile(handle, bytes, size, &read, nullptr))
            return HRESULT_FROM_WIN32(GetLastError());
        if (read != size) return DWRITE_E_FILEFORMAT;
        BYTE actual[32]{};
        const BYTE heading_digest[32] = {
            0x31,0x82,0x98,0x03,0x16,0xd7,0x52,0x7b,0x7e,0xcd,0xac,0x6d,0x78,0x2f,0xd1,0x2a,
            0xa8,0xfb,0x4f,0xf9,0xab,0x9f,0x71,0xe7,0xd0,0x1d,0x1b,0x87,0x33,0x0d,0x26,0xde
        };
        const BYTE toolbar_digest[32] = {0x99, 0x39, 0xae, 0x1e, 0xc7, 0x54, 0x15, 0xc4, 0xb6, 0xe8, 0x88, 0x9c, 0x95, 0x70, 0x4b, 0xfb, 0xcb, 0xfb, 0x9d, 0x42, 0x45, 0x20, 0x36, 0x05, 0xc1, 0x04, 0x4b, 0xa2, 0x18, 0x45, 0xca, 0x36};
        const BYTE tab_digest[32] = {0x8c,0xb6,0x5f,0xe1,0x21,0x08,0x0c,0xb8,0x72,0x1f,0xb9,0xb6,0x43,0xf6,0xb0,0x96,0xd1,0x33,0xd4,0xb7,0x2b,0x85,0x75,0xf6,0x52,0x9e,0xc4,0x58,0x61,0x3d,0xcc,0x78};
        NTSTATUS status = BCryptHash(BCRYPT_SHA256_ALG_HANDLE, nullptr, 0, bytes,
            size, actual, sizeof(actual));
        if (status < 0) return HRESULT_FROM_NT(status);
        return std::memcmp(actual, role == 0 ? heading_digest : role == 1 ? toolbar_digest : tab_digest, sizeof(actual)) == 0 ? S_OK : DWRITE_E_FILEFORMAT;
    }
};

// One short-lived callback per draw. Glyph work remains in DirectWrite.
class Renderer final : public IDWriteTextRenderer {
    LONG references_ = 1;
    IDWriteBitmapRenderTarget* target_;
    IDWriteRenderingParams* parameters_;
    COLORREF color_;
public:
    Renderer(IDWriteBitmapRenderTarget* target, IDWriteRenderingParams* parameters,
             COLORREF color) : target_(target), parameters_(parameters), color_(color) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** result) override {
        if (!result) return E_INVALIDARG;
        *result = nullptr;
        if (iid == __uuidof(IUnknown) || iid == __uuidof(IDWritePixelSnapping)
            || iid == __uuidof(IDWriteTextRenderer)) {
            *result = static_cast<IDWriteTextRenderer*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&references_); }
    ULONG STDMETHODCALLTYPE Release() override {
        ULONG remaining = InterlockedDecrement(&references_);
        if (!remaining) {
            this->~Renderer();
            HeapFree(GetProcessHeap(), 0, this);
        }
        return remaining;
    }
    HRESULT STDMETHODCALLTYPE IsPixelSnappingDisabled(void*, BOOL* disabled) override {
        if (!disabled) return E_INVALIDARG;
        *disabled = TRUE; // Preserve the recovered fractional physical baseline.
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetCurrentTransform(void*, DWRITE_MATRIX* matrix) override {
        return target_->GetCurrentTransform(matrix);
    }
    HRESULT STDMETHODCALLTYPE GetPixelsPerDip(void*, FLOAT* scale) override {
        if (!scale) return E_INVALIDARG;
        *scale = target_->GetPixelsPerDip();
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE DrawGlyphRun(void*, FLOAT x, FLOAT y,
        DWRITE_MEASURING_MODE mode, const DWRITE_GLYPH_RUN* run,
        const DWRITE_GLYPH_RUN_DESCRIPTION*, IUnknown*) override {
        // CreateTextLayout produces natural advances. Refuse a changed contract.
        if (mode != DWRITE_MEASURING_MODE_NATURAL) return E_INVALIDARG;
        return target_->DrawGlyphRun(x, y, mode, run, parameters_, color_, nullptr);
    }
    HRESULT STDMETHODCALLTYPE DrawUnderline(void*, FLOAT, FLOAT,
        const DWRITE_UNDERLINE*, IUnknown*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE DrawStrikethrough(void*, FLOAT, FLOAT,
        const DWRITE_STRIKETHROUGH*, IUnknown*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE DrawInlineObject(void*, FLOAT, FLOAT,
        IDWriteInlineObject*, BOOL, BOOL, IUnknown*) override { return E_NOTIMPL; }
};

struct Context {
    ComPtr<IDWriteFactory5> factory;
    ComPtr<IDWriteInMemoryFontFileLoader> loader;
    bool loader_registered = false;
    ComPtr<IDWriteFontCollection1> collection;
    ComPtr<IDWriteFontFace> faces[3];
    ComPtr<IDWriteTextFormat> formats[3];
    ComPtr<IDWriteGdiInterop> interop;
    ComPtr<IDWriteRenderingParams> parameters;
    ComPtr<IDWriteBitmapRenderTarget> bitmap;
    unsigned bitmap_width = 0;
    unsigned bitmap_height = 0;
    Entry layouts[MaxLayouts];
    unsigned cached_layouts = 0;
    uint64_t clock = 0;

    ~Context() {
        // No font object may outlive the registered in-memory loader.
        for (auto& entry : layouts) entry.layout.Reset();
        bitmap.Reset();
        parameters.Reset();
        interop.Reset();
        for (auto& format : formats) format.Reset();
        for (auto& face : faces) face.Reset();
        collection.Reset();
        if (loader_registered) {
#ifdef SOURCE_TEXT_PILOT_TEST
            ++cleanup_observation.unregister_calls;
            for (const auto& entry : layouts)
                if (entry.layout) ++cleanup_observation.font_objects_at_unregister;
            for (const auto& format : formats)
                if (format) ++cleanup_observation.font_objects_at_unregister;
            for (const auto& face : faces)
                if (face) ++cleanup_observation.font_objects_at_unregister;
            if (collection) ++cleanup_observation.font_objects_at_unregister;
#endif
            HRESULT released = factory->UnregisterFontFileLoader(loader.Get());
#ifdef SOURCE_TEXT_PILOT_TEST
            if (FAILED(released)) ++cleanup_observation.unregister_failures;
#else
            (void)released;
#endif
        }
        loader.Reset();
    }

    HRESULT initialize(const wchar_t* heading_file, const wchar_t* toolbar_file, const wchar_t* tab_file) {
        HRESULT hr = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED,
            __uuidof(IDWriteFactory5), reinterpret_cast<IUnknown**>(factory.GetAddressOf()));
        if (FAILED(hr)) return hr;
        if (FAILED(hr = factory->CreateInMemoryFontFileLoader(&loader))) return hr;
        if (FAILED(hr = factory->RegisterFontFileLoader(loader.Get()))) return hr;
        loader_registered = true;
        ComPtr<IDWriteFontSetBuilder1> builder;
        if (FAILED(hr = factory->CreateFontSetBuilder(&builder))) return hr;
        const wchar_t* files[] = {heading_file, toolbar_file, tab_file};
        for (unsigned role = 0; role < 3; ++role) {
            FontRead source;
            if (FAILED(hr = source.authenticate(files[role], role))) return hr;
            ComPtr<IDWriteFontFile> font_file;
            // SDK contract: null owner copies the same authenticated buffer.
            if (FAILED(hr = loader->CreateInMemoryFontFileReference(factory.Get(),
                source.bytes, source.size, nullptr, &font_file))) return hr;
            if (FAILED(hr = builder->AddFontFile(font_file.Get()))) return hr;
        }
        ComPtr<IDWriteFontSet> set;
        if (FAILED(hr = builder->CreateFontSet(&set))) return hr;
        if (FAILED(hr = factory->CreateFontCollectionFromFontSet(set.Get(), &collection))) return hr;
        const wchar_t* families[] = {L"RazerF5", L"Roboto", L"Roboto"};
        const float sizes[] = {16.0f, 14.0f, 12.0f};
        for (unsigned role = 0; role < 3; ++role) {
            UINT32 index = 0;
            BOOL exists = FALSE;
            if (FAILED(hr = collection->FindFamilyName(families[role], &index, &exists))) return hr;
            if (!exists) return DWRITE_E_NOFONT;
            ComPtr<IDWriteFontFamily> family;
            ComPtr<IDWriteFont> font;
            if (FAILED(hr = collection->GetFontFamily(index, &family))) return hr;
            if (FAILED(hr = family->GetFirstMatchingFont(role == 2 ? DWRITE_FONT_WEIGHT_LIGHT : DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, &font))) return hr;
            if (font->GetSimulations() != DWRITE_FONT_SIMULATIONS_NONE) return DWRITE_E_NOFONT;
            if (font->GetWeight() != (role == 2 ? DWRITE_FONT_WEIGHT_LIGHT : DWRITE_FONT_WEIGHT_NORMAL)) return DWRITE_E_NOFONT;
            if (FAILED(hr = font->CreateFontFace(&faces[role]))) return hr;
            if (FAILED(hr = factory->CreateTextFormat(families[role], collection.Get(),
                role == 2 ? DWRITE_FONT_WEIGHT_LIGHT : DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL, sizes[role], L"en-US", &formats[role]))) return hr;
            if (FAILED(hr = formats[role]->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP))) return hr;
        }
        if (FAILED(hr = factory->GetGdiInterop(&interop))) return hr;
        return factory->CreateRenderingParams(&parameters);
    }

    bool covers(unsigned role, const wchar_t* text, unsigned length) {
        for (unsigned i = 0; i < length; ++i) {
            UINT32 value = text[i];
            if (value >= 0xD800 && value <= 0xDBFF) {
                if (++i >= length || text[i] < 0xDC00 || text[i] > 0xDFFF) return false;
                value = 0x10000 + ((value - 0xD800) << 10) + text[i] - 0xDC00;
            } else if (value >= 0xDC00 && value <= 0xDFFF) return false;
            // This pilot is single-line. Do not silently format control characters.
            if (value < 0x20) return false;
            UINT16 glyph = 0;
            if (FAILED(faces[role]->GetGlyphIndices(&value, 1, &glyph)) || !glyph) return false;
        }
        return true;
    }

    HRESULT layout(unsigned role, const wchar_t* text, unsigned length, Entry** result) {
        // Counter overflow is only possible after2^64 draws; keep bounded ages.
        if (++clock == 0) {
            for (auto& entry : layouts) entry.age = 0;
            clock = 1;
        }
        Entry* candidate = nullptr;
        for (auto& entry : layouts) {
            if (entry.layout && entry.role == role && entry.length == length
                && std::memcmp(entry.text, text, length * sizeof(wchar_t)) == 0) {
                entry.age = clock;
                *result = &entry;
                return S_OK;
            }
            if (!candidate || !entry.layout
                || (candidate->layout && entry.age < candidate->age)) candidate = &entry;
        }
        ComPtr<IDWriteTextLayout> layout;
        HRESULT hr = factory->CreateTextLayout(text, length, formats[role].Get(),
            8192.0f, 256.0f, &layout);
        if (FAILED(hr)) return hr;
        DWRITE_TEXT_METRICS metrics{};
        DWRITE_LINE_METRICS line{};
        UINT32 count = 0;
        if (FAILED(hr = layout->GetMetrics(&metrics))) return hr;
        if (FAILED(hr = layout->GetLineMetrics(&line, 1, &count))) return hr;
        if (!candidate->layout) ++cached_layouts;
        candidate->role = role;
        candidate->length = length;
        candidate->age = clock;
        std::memcpy(candidate->text, text, length * sizeof(wchar_t));
        candidate->layout = std::move(layout);
        candidate->advance = metrics.widthIncludingTrailingWhitespace;
        candidate->baseline = line.baseline;
        *result = candidate;
        return S_OK;
    }
};

unsigned failure(Status status, HRESULT value, HRESULT* error) {
    if (error) *error = value;
    return status;
}
} // namespace

extern "C" unsigned sl_source_text_create(const wchar_t* heading_file, const wchar_t* toolbar_file, const wchar_t* tab_file, void** result,
                                           HRESULT* error) noexcept {
    if (result) *result = nullptr;
    if (!heading_file || !toolbar_file || !tab_file || !result || !error) return failure(InvalidArgument, E_INVALIDARG, error);
    void* storage = HeapAlloc(GetProcessHeap(), 0, sizeof(Context));
    if (!storage) return failure(AllocationFailure, E_OUTOFMEMORY, error);
    auto* context = new (storage) Context;
    HRESULT hr = context->initialize(heading_file, toolbar_file, tab_file);
    if (FAILED(hr)) {
        bool no_factory = !context->factory;
        context->~Context();
        HeapFree(GetProcessHeap(), 0, context);
        return failure(no_factory ? UnsupportedFactory : InvalidFont, hr, error);
    }
    *result = context;
    *error = S_OK;
    return Ok;
}

extern "C" void sl_source_text_destroy(void* context) noexcept {
    if (context) {
        static_cast<Context*>(context)->~Context();
        HeapFree(GetProcessHeap(), 0, context);
    }
}

// Measurement and drawing share the same authenticated face and bounded LRU.
// No DC, bitmap or glyph raster is needed to obtain a natural layout advance.
extern "C" unsigned sl_source_text_measure(void* opaque, unsigned role,
    const wchar_t* text, unsigned length, Metrics* metrics, HRESULT* error) noexcept {
    if (!opaque || role > 2 || !text || !length || length > MaxText || !metrics || !error)
        return failure(InvalidArgument, E_INVALIDARG, error);
    auto* context = static_cast<Context*>(opaque);
    if (!context->covers(role, text, length)) return failure(MissingGlyph, DWRITE_E_NOFONT, error);
    Entry* entry = nullptr;
    HRESULT hr = context->layout(role, text, length, &entry);
    if (FAILED(hr)) return failure(LayoutFailure, hr, error);
    *metrics = {entry->advance, entry->baseline, 0.0f, context->cached_layouts};
    *error = S_OK;
    return Ok;
}

// Clip coordinates are physical pixels; source box coordinates are CSS pixels.
// Role0: assignment RazerF5 16/400, left aligned, source baseline15. Role1: Roboto14/400,
// centered in the source box, source baseline23. Role2: Roboto12/300 host
// label left aligned, baseline11 from its14px title box. No general styling interface.
// Compensate for clip rounding so the absolute source baseline stays fractional.
extern "C" unsigned sl_source_text_draw(void* opaque, HDC destination, unsigned role,
    const wchar_t* text, unsigned length, int clip_x, int clip_y,
    unsigned width, unsigned height, float box_left_css, float box_top_css, float box_width_css, float scale,
    COLORREF color, Metrics* metrics, HRESULT* error) noexcept {
    if (!opaque || !destination || role > 2 || !text || !length || length > MaxText || !metrics
        || !error || !width || !height || width > MaxWidth || height > MaxHeight
        || !std::isfinite(scale) || scale < 0.5f || scale > 4.0f
        || !std::isfinite(box_left_css) || !std::isfinite(box_top_css)
        || !std::isfinite(box_width_css) || box_width_css <= 0.0f || box_width_css > 8192.0f
        || std::fabs(box_left_css) > 32768.0f || std::fabs(box_top_css) > 32768.0f)
        return failure(InvalidArgument, E_INVALIDARG, error);
    auto* context = static_cast<Context*>(opaque);
    {
        if (!context->covers(role, text, length)) return failure(MissingGlyph, DWRITE_E_NOFONT, error);
        Entry* entry = nullptr;
        HRESULT hr = context->layout(role, text, length, &entry);
        if (FAILED(hr)) return failure(LayoutFailure, hr, error);
        if (!context->bitmap) {
            hr = context->interop->CreateBitmapRenderTarget(destination, width, height, &context->bitmap);
        } else if (width != context->bitmap_width || height != context->bitmap_height) {
            hr = context->bitmap->Resize(width, height);
        }
        if (FAILED(hr)) return failure(BitmapFailure, hr, error);
        context->bitmap_width = width;
        context->bitmap_height = height;
        if (FAILED(hr = context->bitmap->SetPixelsPerDip(scale))) return failure(BitmapFailure, hr, error);
        HDC scratch = context->bitmap->GetMemoryDC();
        if (!BitBlt(scratch, 0, 0, width, height, destination, clip_x, clip_y, SRCCOPY))
            return failure(BlitFailure, HRESULT_FROM_WIN32(GetLastError()), error);
        ComPtr<Renderer> renderer;
        void* renderer_storage = HeapAlloc(GetProcessHeap(), 0, sizeof(Renderer));
        if (!renderer_storage) return failure(AllocationFailure, E_OUTOFMEMORY, error);
        renderer.Attach(new (renderer_storage) Renderer(context->bitmap.Get(), context->parameters.Get(), color));
        const float source_baseline = box_top_css + (role == 0 ? 15.0f : role == 1 ? 23.0f : 11.0f);
        const float x_css = box_left_css + (role == 1 ? (box_width_css - entry->advance) / 2.0f : 0.0f)
            - static_cast<float>(clip_x) / scale;
        const float baseline_css = source_baseline - static_cast<float>(clip_y) / scale;
        hr = entry->layout->Draw(nullptr, renderer.Get(), x_css, baseline_css - entry->baseline);
        if (FAILED(hr)) return failure(RenderFailure, hr, error);
        // Destination remains unchanged on every prior failure.
        if (!BitBlt(destination, clip_x, clip_y, width, height, scratch, 0, 0, SRCCOPY))
            return failure(BlitFailure, HRESULT_FROM_WIN32(GetLastError()), error);
        *metrics = {entry->advance, entry->baseline, source_baseline * scale,
                    context->cached_layouts};
        *error = S_OK;
        return Ok;
    }
}
