// Standalone metrics diagnostic: see docs/TEXT-METRICS.md. No application or device I/O.
#include <windows.h>
#include <dwrite_3.h>
#include <wrl/client.h>
#include <iostream>
#include <iomanip>
#include <cmath>
#include <string>
using Microsoft::WRL::ComPtr;
void check(HRESULT hr) { if (FAILED(hr)) { std::cerr << "HRESULT " << std::hex << hr; ExitProcess(1); } }
struct Row { const char* id; const wchar_t* family; const wchar_t* text; float size; float line; };
int wmain(int argc, wchar_t** argv) {
    if (argc != 2) return 2;
    ComPtr<IDWriteFactory5> factory;
    check(DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED, __uuidof(IDWriteFactory5), reinterpret_cast<IUnknown**>(factory.GetAddressOf())));
    ComPtr<IDWriteFontSetBuilder1> builder; check(factory->CreateFontSetBuilder(&builder));
    for (const auto* name : { L"Roboto-Regular.ttf", L"RazerF5-Regular.ttf" }) {
        ComPtr<IDWriteFontFile> file; std::wstring path = std::wstring(argv[1]) + L"\\" + name;
        check(factory->CreateFontFileReference(path.c_str(), nullptr, &file)); check(builder->AddFontFile(file.Get()));
    }
    ComPtr<IDWriteFontSet> set; check(builder->CreateFontSet(&set));
    ComPtr<IDWriteFontCollection1> collection; check(factory->CreateFontCollectionFromFontSet(set.Get(), &collection));
    Row rows[] = {
        {"header",L"Roboto",L"A",14,17},
        {"category",L"Roboto",L"MOUSE FUNCTION",12,17},
        {"heading",L"RazerF5",L"MOUSE FUNCTION",16,19},
        {"default",L"Roboto",L"A",14,17},
        {"option",L"Roboto",L"Double Click",14,17},
        {"options",L"Roboto",L"Double Click",14,17},
        {"turbo",L"Roboto",L"Speed (press/sec)",14,17},
        {"button",L"Roboto",L"CANCEL",12,17},
        {"layer",L"Roboto",L"Standard",14,17},
        {"mouse",L"Roboto",L"Right Click",14,17},
        {"kerning",L"Roboto",L"AVATAR To fi ffi",14,17},
        {"razer-kerning",L"RazerF5",L"AVATAR To fi ffi",16,19},
        {"ornata-title",L"Roboto",L"RAZER ORNATA V2",14,17},
        {"deathadder-title",L"Roboto",L"RAZER DEATHADDER V3 HYPERSPEED",14,17},
        {"roboto-avatar",L"Roboto",L"AVATAR",14,17},
        {"roboto-to",L"Roboto",L"To",14,17},
        {"roboto-fi",L"Roboto",L"fi",14,17},
        {"roboto-ffi",L"Roboto",L"ffi",14,17},
        {"razerf5-avatar",L"RazerF5",L"AVATAR",16,19},
        {"razerf5-to",L"RazerF5",L"To",16,19},
        {"razerf5-fi",L"RazerF5",L"fi",16,19},
        {"razerf5-ffi",L"RazerF5",L"ffi",16,19}
    };
    std::cout << std::setprecision(10) << "{\"api\":\"DirectWrite isolated factory5, explicit local-only font collection\",\"rows\":[";
    bool first=true;
    for (float scale : {1.0f,1.25f}) for (const auto& row : rows) for (int mode=0; mode<7; ++mode) {
        ComPtr<IDWriteTextFormat> format; check(factory->CreateTextFormat(row.family, collection.Get(), DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,row.size,L"en-US",&format));
        ComPtr<IDWriteTextLayout> layout;
        if(mode==0 || mode>=3) check(factory->CreateTextLayout(row.text,UINT32(wcslen(row.text)),format.Get(),2000,200,&layout));
        else check(factory->CreateGdiCompatibleTextLayout(row.text,UINT32(wcslen(row.text)),format.Get(),2000,200,scale,nullptr,mode==2,&layout));
        if (mode==3) { ComPtr<IDWriteTextLayout1> layout1; check(layout.As(&layout1)); check(layout1->SetPairKerning(TRUE,DWRITE_TEXT_RANGE{0,UINT32(wcslen(row.text))})); }
        if (mode>=4) {
            ComPtr<IDWriteTypography> typography;
            check(factory->CreateTypography(&typography));
            check(typography->AddFontFeature(DWRITE_FONT_FEATURE{DWRITE_FONT_FEATURE_TAG_KERNING,mode==5?0u:1u}));
            check(typography->AddFontFeature(DWRITE_FONT_FEATURE{DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES,mode==4?0u:1u}));
            check(typography->AddFontFeature(DWRITE_FONT_FEATURE{DWRITE_FONT_FEATURE_TAG_CONTEXTUAL_LIGATURES,mode==4?0u:1u}));
            check(layout->SetTypography(typography.Get(),DWRITE_TEXT_RANGE{0,UINT32(wcslen(row.text))}));
        }
        DWRITE_TEXT_METRICS metrics{}; check(layout->GetMetrics(&metrics));
        DWRITE_LINE_METRICS line{}; UINT32 count=0; check(layout->GetLineMetrics(&line,1,&count));
        ComPtr<IDWriteFontFamily> family; UINT32 index=0; BOOL exists=false;check(collection->FindFamilyName(row.family,&index,&exists));if(!exists)return 3; check(collection->GetFontFamily(index,&family));
        ComPtr<IDWriteFont> font; check(family->GetFirstMatchingFont(DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STRETCH_NORMAL,DWRITE_FONT_STYLE_NORMAL,&font));
        DWRITE_FONT_METRICS fm{};font->GetMetrics(&fm);
        ComPtr<IDWriteFontFace> face; check(font->CreateFontFace(&face));
        UINT32 missing = 0;
        for (const auto* character = row.text; *character; ++character) {
            UINT32 codepoint = *character; UINT16 glyph = 0;
            check(face->GetGlyphIndices(&codepoint,1,&glyph));
            if (glyph == 0) ++missing;
        }
        if(!first) std::cout << ',';first=false;
        std::cout
            << "{\"id\":\""
            << row.id
            << "\",\"scale\":"
            << scale
            << ",\"mode\":\""
            << (mode==0?"natural":mode==1?"gdi-classic":mode==2?"gdi-natural":mode==3?"natural-pair-kerning":mode==4?"natural-no-ligatures":mode==5?"natural-no-kerning":"natural-explicit-features")
            << "\",\"font_size_css\":"
            << row.size
            << ",\"weight\":400,\"advance_css\":"
            << metrics.widthIncludingTrailingWhitespace
            << ",\"advance_physical\":"
            << metrics.widthIncludingTrailingWhitespace*scale
            << ",\"default_line_height_css\":"
            << line.height
            << ",\"default_baseline_css\":"
            << line.baseline
            << ",\"missing_glyphs\":"
            << missing
            << ",\"font_simulations\":"
            << font->GetSimulations()
            << ",\"font_metrics\":{\"units_per_em\":"
            << fm.designUnitsPerEm
            << ",\"ascent\":"
            << fm.ascent
            << ",\"descent\":"
            << fm.descent
            << ",\"line_gap\":"
            << fm.lineGap
            << "}}";
    }
    std::cout << "]}";
}
