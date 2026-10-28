// Standalone bounded SDK smoke; simulated text only, no windows/devices.
#include "../crates/secbird-tray/src/native_editor/source_text_bridge.cpp"
#include <iostream>
#include <string>
#include <cassert>
int wmain(int argc, wchar_t** argv) {
    assert(argc == 2);
    const std::wstring heading_file = std::wstring(argv[1]) + L"\\RazerF5-Regular.ttf";
    const std::wstring toolbar_file = std::wstring(argv[1]) + L"\\Roboto-Regular.ttf";
    const std::wstring tab_file = std::wstring(argv[1]) + L"\\Roboto-Light.ttf";
    const std::wstring source_files[] = {heading_file, toolbar_file, tab_file};
    DWORD gdi_before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
    wchar_t temp_directory[MAX_PATH], temporary[3][MAX_PATH];
    assert(GetTempPathW(MAX_PATH,temp_directory));
    for (unsigned role=0;role<3;++role) {
        assert(GetTempFileNameW(temp_directory,L"SLT",0,temporary[role]));
        assert(CopyFileW(source_files[role].c_str(),temporary[role],FALSE));
    }
    void* opaque = nullptr; HRESULT hr = S_OK;
    assert(sl_source_text_create(temporary[0],temporary[1],temporary[2],&opaque,&hr)==Ok);
    // All three authenticated paths disappear before any glyph is drawn.
    for (auto& file:temporary) assert(DeleteFileW(file));
    HDC dc = CreateCompatibleDC(nullptr);
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = 512; info.bmiHeader.biHeight = -512;
    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
    void* pixels = nullptr;
    HBITMAP bitmap = CreateDIBSection(dc,&info,DIB_RGB_COLORS,&pixels,nullptr,0);
    auto previous = SelectObject(dc,bitmap);
    RECT bounds{0,0,512,512};
    FillRect(dc,&bounds,static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
    Metrics result{};
    const wchar_t* text=L"MOUSE FUNCTION";
    assert(sl_source_text_draw(opaque,dc,0,text,UINT32(wcslen(text)),0,0,512,64,0,0,400,1.25f,RGB(68,214,44),&result,&hr)==Ok);
    assert(std::fabs(result.advance_css*1.25f-159.14f)<0.0002f);
    assert(result.source_baseline_physical==18.75f);
    unsigned changed=0;
    for(unsigned i=0;i<512*64;++i) if(static_cast<unsigned*>(pixels)[i]&0xFFFFFF) ++changed;
    assert(changed>0);
    // CSS top189 -> physical236.25, but the clip starts at integer236.
    // The bridge must compensate that rounding and keep baseline255 exact.
    assert(sl_source_text_draw(opaque,dc,0,text,UINT32(wcslen(text)),12,236,500,64,10,189,400,1.25f,RGB(68,214,44),&result,&hr)==Ok);
    assert(result.source_baseline_physical==255.0f);
    const wchar_t* title=L"RAZER DEATHADDER V3 HYPERSPEED";
    assert(sl_source_text_draw(opaque,dc,1,title,UINT32(wcslen(title)),0,0,512,64,0,0,400,1.25f,RGB(153,153,153),&result,&hr)==Ok);
    assert(std::fabs(result.advance_css*1.25f-304.541015625f)<0.0002f);
    assert(result.source_baseline_physical==28.75f);
    assert(result.cached_layouts==2);
    const wchar_t* tab=L"SYNAPSE";
    assert(sl_source_text_measure(opaque,2,tab,UINT32(wcslen(tab)),&result,&hr)==Ok);
    assert(std::fabs(result.advance_css-51.732421875f)<0.00001f);
    assert(sl_source_text_draw(opaque,dc,2,tab,UINT32(wcslen(tab)),54,23,250,30,43.25f,18.25f,200,1.25f,RGB(204,204,204),&result,&hr)==Ok);
    assert(result.source_baseline_physical==36.5625f);
    assert(result.cached_layouts==3);
    for(unsigned i=0;i<40;++i) {
        std::wstring name=L"HEADING "+std::to_wstring(i);
        assert(sl_source_text_draw(opaque,dc,i%2,name.c_str(),UINT32(name.size()),0,0,512,64,0,0,400,1,RGB(68,214,44),&result,&hr)==Ok);
    }
    assert(result.cached_layouts==32);
    auto* context=static_cast<Context*>(opaque);
    assert(context->bitmap_width==512 && context->bitmap_height==64);
    unsigned before=0;for(unsigned i=0;i<512*512;++i)before^=static_cast<unsigned*>(pixels)[i];
    assert(sl_source_text_draw(opaque,dc,0,text,14,0,0,2049,64,0,0,400,1,0,&result,&hr)==InvalidArgument);
    wchar_t invalid[]={0xD800};
    assert(sl_source_text_draw(opaque,dc,0,invalid,1,0,0,512,64,0,0,400,1,0,&result,&hr)==MissingGlyph);
    assert(sl_source_text_measure(opaque,3,tab,UINT32(wcslen(tab)),&result,&hr)==InvalidArgument);
    unsigned after=0;for(unsigned i=0;i<512*512;++i)after^=static_cast<unsigned*>(pixels)[i];
    assert(before==after);
    sl_source_text_destroy(opaque);
    void* missing=nullptr;
    assert(sl_source_text_create(L"missing-font-test.ttf",toolbar_file.c_str(),tab_file.c_str(),&missing,&hr)!=Ok && missing==nullptr);
    for (unsigned role=0;role<3;++role) {
        assert(CopyFileW(source_files[role].c_str(),temporary[role],FALSE));
        HANDLE altered=CreateFileW(temporary[role],GENERIC_WRITE,0,nullptr,OPEN_EXISTING,0,nullptr);
        assert(altered!=INVALID_HANDLE_VALUE);
        BYTE bad=0xFF;DWORD written=0;
        assert(WriteFile(altered,&bad,1,&written,nullptr));CloseHandle(altered);
        assert(sl_source_text_create(role==0?temporary[role]:heading_file.c_str(),role==1?temporary[role]:toolbar_file.c_str(),role==2?temporary[role]:tab_file.c_str(),&missing,&hr)==InvalidFont && missing==nullptr);
        assert(DeleteFileW(temporary[role]));
    }
    SelectObject(dc,previous);DeleteObject(bitmap);DeleteDC(dc);
    assert(GetGuiResources(GetCurrentProcess(),GR_GDIOBJECTS)==gdi_before);
#ifdef SOURCE_TEXT_PILOT_TEST
    assert(cleanup_observation.unregister_calls==5);
    assert(cleanup_observation.unregister_failures==0);
    assert(cleanup_observation.font_objects_at_unregister==0);
#endif
    std::cout << "{\"natural_heading_advance_physical\":159.14,\"natural_toolbar_advance_physical\":304.541015625,\"source_heading_baseline_physical\":18.75,\"source_toolbar_baseline_physical\":28.75,\"source_offset_baseline_physical\":255,\"nonempty_offscreen_glyphs\":true,\"total_role_aware_layout_cache_after40\":32,\"scratch_size\":[512,64],\"invalid_requests_leave_destination_unchanged\":true,\"missing_font_visible_error\":true,\"same_authenticated_bytes_after_three_paths_deleted\":true,\"altered_same_size_font_refused_three_roles\":true,\"gdi_handles_restored\":true,\"screenshots\":false,\"production_linked\":false}";
}
