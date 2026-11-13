use windows_sys::Win32::{Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, TRUE, WPARAM}, Graphics::Gdi::*, UI::{Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_HOTLIGHT, ODS_SELECTED, ODT_MENU}, Shell::{DefSubclassProc, SetWindowSubclass}, WindowsAndMessaging::*}};
const ID: usize = 0x4d385042;
const TAG: usize = 0x504c5553;
const fn rgb(r: u32, g: u32, b: u32) -> u32 { r | g << 8 | b << 16 }
const PAD: i32 = 12;
fn label() -> Vec<u16> { crate::menu::PLUS_TITLE.encode_utf16().collect() }
unsafe fn menu_font() -> HFONT {
    let mut m: NONCLIENTMETRICSW = unsafe { std::mem::zeroed() }; m.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
    unsafe { SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, m.cbSize, &mut m as *mut _ as _, 0); CreateFontIndirectW(&m.lfMenuFont) }
}
unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, _: usize) -> LRESULT {
    match msg {
        WM_MEASUREITEM => { let mi = unsafe { &mut *(lp as *mut MEASUREITEMSTRUCT) };
            if mi.CtlType == ODT_MENU && mi.itemData == TAG { unsafe {
                let (dc, font, text) = (GetDC(hwnd), menu_font(), label()); let old = SelectObject(dc, font);
                let mut sz = SIZE { cx: 0, cy: 0 }; GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut sz);
                SelectObject(dc, old); DeleteObject(font); ReleaseDC(hwnd, dc);
                mi.itemWidth = (sz.cx + 2 * PAD) as u32; mi.itemHeight = sz.cy as u32;
            } return TRUE as LRESULT; } }
        WM_DRAWITEM => { let di = unsafe { &*(lp as *const DRAWITEMSTRUCT) };
            if di.CtlType == ODT_MENU && di.itemData == TAG { unsafe {
                let (dc, mut r, text) = (di.hDC, di.rcItem, label());
                FillRect(dc, &r, GetSysColorBrush(COLOR_MENUBAR));
                let hot = di.itemState & (ODS_HOTLIGHT | ODS_SELECTED) != 0;
                let (brush, pen) = (CreateSolidBrush(if hot { rgb(210, 70, 210) } else { rgb(170, 0, 170) }), CreatePen(PS_SOLID, 1, rgb(85, 0, 85)));
                let (ob, op) = (SelectObject(dc, brush), SelectObject(dc, pen));
                RoundRect(dc, r.left + 2, r.top + 2, r.right - 2, r.bottom - 1, 8, 8);
                SelectObject(dc, ob); SelectObject(dc, op); DeleteObject(brush); DeleteObject(pen);
                let font = menu_font(); let of = SelectObject(dc, font);
                SetBkMode(dc, TRANSPARENT as i32); SetTextColor(dc, rgb(255, 255, 255));
                DrawTextW(dc, text.as_ptr(), text.len() as i32, &mut r as *mut RECT, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
                SelectObject(dc, of); DeleteObject(font);
            } return TRUE as LRESULT; } }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}
pub(super) fn attach(hwnd: HWND) {
    unsafe {
        let bar = GetMenu(hwnd); let n = GetMenuItemCount(bar); if n <= 0 { return; }
        let mut mii: MENUITEMINFOW = std::mem::zeroed(); mii.cbSize = size_of::<MENUITEMINFOW>() as u32; mii.fMask = MIIM_FTYPE;
        if GetMenuItemInfoW(bar, (n - 1) as u32, TRUE, &mut mii) == 0 { return; }
        mii.fMask = MIIM_FTYPE | MIIM_DATA; mii.fType |= MFT_OWNERDRAW; mii.dwItemData = TAG;
        if SetMenuItemInfoW(bar, (n - 1) as u32, TRUE, &mii) != 0 && SetWindowSubclass(hwnd, Some(subclass), ID, 0) != 0 { DrawMenuBar(hwnd); }
    }
}
