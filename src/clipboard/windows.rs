use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::slice;

use windows_sys::Win32::Foundation::{GlobalFree, HGLOBAL, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatA, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_MESSAGE, WS_CHILD,
};

use super::{AccessError, ClipboardBackend, ClipboardText, Generation, Publication};

const TEXT_FORMAT: u32 = CF_UNICODETEXT as u32;

#[derive(Default)]
pub(crate) struct SystemClipboard;

struct ClipboardWindow(HWND);

impl ClipboardWindow {
    fn create() -> Option<Self> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
        // Use the system's STATIC class for a message-only window on this thread.
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                ptr::null(),
                WS_CHILD,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        (!window.is_null()).then(|| Self(window))
    }
}

impl Drop for ClipboardWindow {
    fn drop(&mut self) {
        // This thread created the window and has already closed the clipboard.
        unsafe { DestroyWindow(self.0) };
    }
}

struct ClipboardLock;

impl ClipboardLock {
    fn open(window: HWND) -> Option<Self> {
        // A publisher passes its own valid HWND. Readers can pass NULL.
        (unsafe { OpenClipboard(window) } != 0).then(|| Self)
    }

    fn clear(&self) -> bool {
        // The clipboard remains exclusively open on this thread.
        (unsafe { EmptyClipboard() }) != 0
    }

    fn write(&self, format: u32, memory: OwnedMemory) -> bool {
        // This thread holds the clipboard open. On success Win32 owns the block.
        if unsafe { SetClipboardData(format, memory.0) }.is_null() {
            return false;
        }
        std::mem::forget(memory);
        true
    }

    fn read(&self, format: u32) -> Result<Option<String>, AccessError> {
        // The data handle is valid only while this clipboard lock is held.
        if unsafe { IsClipboardFormatAvailable(format) } == 0 {
            return Ok(None);
        }
        let handle = unsafe { GetClipboardData(format) };
        if handle.is_null() {
            return Err(AccessError::Unavailable);
        }

        let size = unsafe { GlobalSize(handle) };
        if !(size_of::<u16>()..=isize::MAX as usize).contains(&size) {
            return Err(AccessError::Unavailable);
        }
        let memory = LockedMemory::lock(handle).ok_or(AccessError::Unavailable)?;
        let start = memory.units.as_ptr();
        // Global allocations are aligned for UTF-16. Stop before allocation padding.
        let end = (0..size / size_of::<u16>())
            .find(|&index| unsafe { *start.add(index) == 0 })
            .ok_or(AccessError::Unavailable)?;
        let units = unsafe { slice::from_raw_parts(start, end) };
        String::from_utf16(units)
            .map(Some)
            .map_err(|_| AccessError::Unavailable)
    }
}

impl Drop for ClipboardLock {
    fn drop(&mut self) {
        // Paired with the successful OpenClipboard on this thread.
        unsafe { CloseClipboard() };
    }
}

struct OwnedMemory(HGLOBAL);

impl OwnedMemory {
    fn utf16(text: &str) -> Option<Self> {
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        // Win32 clipboard formats require a movable global allocation.
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(units.as_slice())) };
        if handle.is_null() {
            return None;
        }
        let memory = Self(handle);
        {
            let locked = LockedMemory::lock(handle)?;
            // The new allocation has space for all UTF-16 units, including the terminator.
            unsafe { ptr::copy_nonoverlapping(units.as_ptr(), locked.units.as_ptr(), units.len()) };
        }
        Some(memory)
    }
}

impl Drop for OwnedMemory {
    fn drop(&mut self) {
        // This block has not been transferred to the clipboard.
        unsafe { GlobalFree(self.0) };
    }
}

struct LockedMemory {
    handle: HGLOBAL,
    units: NonNull<u16>,
}

impl LockedMemory {
    fn lock(handle: HGLOBAL) -> Option<Self> {
        // The caller owns or borrows a valid global memory handle until this guard drops.
        let units = NonNull::new(unsafe { GlobalLock(handle) }.cast())?;
        Some(Self { handle, units })
    }
}

impl Drop for LockedMemory {
    fn drop(&mut self) {
        // Paired with the successful GlobalLock.
        unsafe { GlobalUnlock(self.handle) };
    }
}

fn token_format() -> Option<u32> {
    // The format name is ASCII and nul-terminated.
    let format = unsafe { RegisterClipboardFormatA(c"RustXL Clipboard Token".as_ptr().cast()) };
    (format != 0).then_some(format)
}

fn sequence_number() -> Option<Generation> {
    // Win32 returns zero when the sequence number is unavailable.
    let generation = unsafe { GetClipboardSequenceNumber() };
    (generation != 0).then_some(generation as Generation)
}

impl ClipboardBackend for SystemClipboard {
    fn publish(&mut self, text: &str, token: &str) -> Publication {
        let Some(text) = OwnedMemory::utf16(text) else {
            return Publication::Failed;
        };
        let token = token_format().zip(OwnedMemory::utf16(token));
        let Some(window) = ClipboardWindow::create() else {
            return Publication::Offline;
        };
        let Some(clipboard) = ClipboardLock::open(window.0) else {
            return Publication::Failed;
        };
        if !clipboard.clear() || !clipboard.write(TEXT_FORMAT, text) {
            return Publication::Failed;
        }

        let marker_written = token.is_some_and(|(format, memory)| clipboard.write(format, memory));
        // Sample after both writes, while other processes are still excluded.
        let receipt = if marker_written {
            sequence_number()
        } else {
            None
        };
        drop(clipboard);
        // All formats were rendered immediately, so the owner window can go away.
        Publication::Published(receipt)
    }

    fn generation(&mut self) -> Option<Generation> {
        sequence_number()
    }

    fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError> {
        let Some(clipboard) = ClipboardLock::open(ptr::null_mut()) else {
            return Err(if ClipboardWindow::create().is_some() {
                AccessError::Unavailable
            } else {
                AccessError::Offline
            });
        };
        let Some(text) = clipboard.read(TEXT_FORMAT)? else {
            return Ok(None);
        };
        let token = match token_format() {
            Some(format) => clipboard.read(format).ok().flatten(),
            None => None,
        };
        Ok(Some(ClipboardText { text, token }))
    }
}

#[cfg(test)]
#[test]
#[ignore = "replaces the clipboard in a disposable Windows desktop session"]
fn windows_clipboard_moves_only_the_current_publication() {
    use crate::clipboard::ClipboardRead;
    use crate::spreadsheet::Spreadsheet;

    let mut clipboard = SystemClipboard;
    let mut sheet = Spreadsheet::new();
    sheet.set_cell(0, 0, "café".into());
    sheet.set_cell_bold(0, 0, true);
    sheet.cut_selection(&mut clipboard);
    sheet.cursor_col = 1;
    sheet.paste(&mut clipboard);
    assert_eq!(sheet.get_cell(0, 0), "");
    assert_eq!(sheet.get_cell(0, 1), "café");
    assert!(sheet.get_cell_style(0, 1).bold);

    sheet.cut_selection(&mut clipboard);
    let ClipboardRead::Text(entry, Some(first)) = clipboard.read_stable() else {
        panic!("expected a stable Windows publication with a receipt");
    };
    let token = entry
        .token
        .expect("expected the registered clipboard format");
    let Publication::Published(Some(republished)) = SystemClipboard.publish(&entry.text, &token)
    else {
        panic!("expected a Windows receipt for the identical republication");
    };
    assert_ne!(first, republished);

    for col in [2, 3] {
        sheet.cursor_col = col;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 1), "café");
        assert!(sheet.get_cell_style(0, 1).bold);
        assert_eq!(sheet.get_cell(0, col), "café");
        assert!(!sheet.get_cell_style(0, col).bold);
    }
}
