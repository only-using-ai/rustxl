pub(crate) type Generation = isize;

pub(crate) enum Publication {
    Offline,
    Failed,
    Published(Option<Generation>),
}

#[cfg_attr(test, derive(Clone))]
pub(crate) struct ClipboardText {
    pub text: String,
    pub token: Option<String>,
}

pub(crate) enum ClipboardRead {
    Offline,
    Unavailable,
    Changed,
    NoText,
    Text(ClipboardText, Option<Generation>),
}

#[derive(Clone, Copy)]
pub(crate) enum AccessError {
    Offline,
    Unavailable,
}

pub(crate) trait ClipboardBackend {
    fn publish(&mut self, text: &str, token: &str) -> Publication;
    fn generation(&mut self) -> Option<Generation>;
    fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError>;

    fn read_stable(&mut self) -> ClipboardRead {
        let before = self.generation();
        let contents = self.read_text();
        let after = self.generation();

        if before != after {
            return ClipboardRead::Changed;
        }
        match contents {
            Ok(Some(text)) => ClipboardRead::Text(text, after),
            Ok(None) => ClipboardRead::NoText,
            Err(AccessError::Offline) => ClipboardRead::Offline,
            Err(AccessError::Unavailable) => ClipboardRead::Unavailable,
        }
    }
}

pub(crate) fn export_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_EXPORT: AtomicU64 = AtomicU64::new(0);
    let serial = NEXT_EXPORT.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}-{nanos:x}-{serial:x}", std::process::id())
}

pub(crate) use platform::SystemClipboard;

#[cfg(windows)]
#[path = "clipboard/windows.rs"]
mod platform;

#[cfg(target_os = "macos")]
mod platform {
    use objc2::rc::{Retained, autoreleasepool};
    use objc2::{ClassType, msg_send};
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::{NSArray, NSString};

    use super::{AccessError, ClipboardBackend, ClipboardText, Generation, Publication};

    const TOKEN_TYPE: &str = "com.rustxl.clipboard-token";

    #[derive(Default)]
    pub(crate) struct SystemClipboard {
        board: Option<Retained<NSPasteboard>>,
    }

    fn general_pasteboard() -> Option<Retained<NSPasteboard>> {
        // AppKit can return nil outside a graphical session.
        unsafe { msg_send![NSPasteboard::class(), generalPasteboard] }
    }

    impl SystemClipboard {
        fn board(&mut self) -> Option<&NSPasteboard> {
            if self.board.is_none() {
                self.board = general_pasteboard();
            }
            self.board.as_deref()
        }
    }

    impl ClipboardBackend for SystemClipboard {
        fn publish(&mut self, text: &str, token: &str) -> Publication {
            autoreleasepool(|_| {
                let Some(board) = self.board() else {
                    return Publication::Offline;
                };
                let token_type = NSString::from_str(TOKEN_TYPE);
                // AppKit exports this immutable constant for the standard text type.
                let text_type = unsafe { NSPasteboardTypeString };
                let types = NSArray::from_slice(&[text_type, &*token_type]);
                // Capture the ownership count from the declaration itself. Values are supplied
                // synchronously, so no owner object or callback is needed.
                let generation = unsafe { board.declareTypes_owner(&types, None) };
                if board.setString_forType(&NSString::from_str(text), text_type)
                    && board.setString_forType(&NSString::from_str(token), &token_type)
                    && board.changeCount() == generation
                {
                    Publication::Published(Some(generation))
                } else {
                    Publication::Failed
                }
            })
        }

        fn generation(&mut self) -> Option<Generation> {
            autoreleasepool(|_| self.board().map(NSPasteboard::changeCount))
        }

        fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError> {
            autoreleasepool(|_| {
                let board = self.board().ok_or(AccessError::Offline)?;
                let Some(items) = board.pasteboardItems() else {
                    return Err(AccessError::Unavailable);
                };
                // AppKit exports this immutable constant for the standard text type.
                let text_type = unsafe { NSPasteboardTypeString };
                let token_type = NSString::from_str(TOKEN_TYPE);
                for index in 0..items.count() {
                    let item = items.objectAtIndex(index);
                    if let Some(text) = item.stringForType(text_type) {
                        let token = item
                            .stringForType(&token_type)
                            .map(|token| token.to_string());
                        return Ok(Some(ClipboardText {
                            text: text.to_string(),
                            token,
                        }));
                    }
                }
                Ok(None)
            })
        }
    }

    #[cfg(test)]
    #[test]
    #[ignore = "requires access to a macOS pasteboard server"]
    fn native_pasteboard_keeps_plain_text_and_detects_identical_republication() {
        struct IsolatedBoard(Retained<NSPasteboard>);

        impl Drop for IsolatedBoard {
            fn drop(&mut self) {
                // Only the test's uniquely named pasteboard is released.
                unsafe { msg_send![&*self.0, releaseGlobally] }
            }
        }

        let board = IsolatedBoard(NSPasteboard::pasteboardWithUniqueName());
        let mut clipboard = SystemClipboard {
            board: Some(board.0.clone()),
        };
        let text = "café\t\n\tlast\t";
        let Publication::Published(Some(first)) = clipboard.publish(text, "marker") else {
            panic!("initial native clipboard publication failed");
        };
        let super::ClipboardRead::Text(entry, Some(observed)) = clipboard.read_stable() else {
            panic!("the native clipboard entry was unreadable");
        };
        assert_eq!(entry.text, text);
        assert_eq!(entry.token.as_deref(), Some("marker"));
        assert_eq!(observed, first);

        let Publication::Published(Some(republished)) = clipboard.publish(text, "marker") else {
            panic!("native clipboard republication failed");
        };
        assert_ne!(republished, first);
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::{AccessError, ClipboardBackend, ClipboardText, Generation, Publication};

    const TOKEN_PREFIX: &str = "<!-- rustxl-token:";
    const TOKEN_SUFFIX: &str = " -->";

    #[derive(Default)]
    pub(crate) struct SystemClipboard {
        handle: Option<arboard::Clipboard>,
    }

    impl SystemClipboard {
        fn handle(&mut self) -> Option<&mut arboard::Clipboard> {
            if self.handle.is_none() {
                self.handle = arboard::Clipboard::new().ok();
            }
            self.handle.as_mut()
        }
    }

    fn companion_html(text: &str, token: &str) -> String {
        let mut html = format!("{TOKEN_PREFIX}{token}{TOKEN_SUFFIX}<table>");
        for line in text.split('\n') {
            html.push_str("<tr>");
            for field in line.split('\t') {
                html.push_str("<td>");
                for character in field.chars() {
                    match character {
                        '&' => html.push_str("&amp;"),
                        '<' => html.push_str("&lt;"),
                        '>' => html.push_str("&gt;"),
                        _ => html.push(character),
                    }
                }
                html.push_str("</td>");
            }
            html.push_str("</tr>");
        }
        html.push_str("</table>");
        html
    }

    impl ClipboardBackend for SystemClipboard {
        fn publish(&mut self, text: &str, token: &str) -> Publication {
            let Some(handle) = self.handle() else {
                return Publication::Offline;
            };
            let html = companion_html(text, token);
            match handle.set_html(html.as_str(), Some(text)) {
                Ok(()) => Publication::Published(None),
                Err(_) => Publication::Failed,
            }
        }

        fn generation(&mut self) -> Option<Generation> {
            None
        }

        fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError> {
            let handle = self.handle().ok_or(AccessError::Offline)?;
            let text = match handle.get_text() {
                Ok(text) => text,
                Err(arboard::Error::ContentNotAvailable | arboard::Error::ConversionFailure) => {
                    return Ok(None);
                }
                Err(_) => return Err(AccessError::Unavailable),
            };
            let html = handle.get().html().ok();
            let token = html.as_deref().and_then(|html| {
                let (_, marker) = html.split_once(TOKEN_PREFIX)?;
                let (token, _) = marker.split_once(TOKEN_SUFFIX)?;
                Some(token.to_owned())
            });
            Ok(Some(ClipboardText { text, token }))
        }
    }
}

#[cfg(test)]
pub(crate) struct OfflineClipboard;

#[cfg(test)]
impl ClipboardBackend for OfflineClipboard {
    fn publish(&mut self, _: &str, _: &str) -> Publication {
        Publication::Offline
    }

    fn generation(&mut self) -> Option<Generation> {
        None
    }

    fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError> {
        Err(AccessError::Offline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spreadsheet::Spreadsheet;

    #[derive(Default)]
    struct FakeClipboard {
        generation: Option<Generation>,
        entry: Option<ClipboardText>,
        read_error: Option<AccessError>,
        change_during_read: bool,
        fail_write: bool,
    }

    impl FakeClipboard {
        fn tracked() -> Self {
            Self {
                generation: Some(0),
                ..Self::default()
            }
        }

        fn new_generation(&mut self) {
            self.generation = self.generation.map(|generation| generation + 1);
        }

        fn replace_text(&mut self, text: &str) {
            self.new_generation();
            self.entry = Some(ClipboardText {
                text: text.into(),
                token: None,
            });
        }
    }

    impl ClipboardBackend for FakeClipboard {
        fn publish(&mut self, text: &str, token: &str) -> Publication {
            if self.fail_write {
                return Publication::Failed;
            }
            self.new_generation();
            self.entry = Some(ClipboardText {
                text: text.into(),
                token: Some(token.into()),
            });
            Publication::Published(self.generation)
        }

        fn generation(&mut self) -> Option<Generation> {
            self.generation
        }

        fn read_text(&mut self) -> Result<Option<ClipboardText>, AccessError> {
            if let Some(error) = self.read_error {
                return Err(error);
            }
            let entry = self.entry.clone();
            if std::mem::take(&mut self.change_during_read) {
                self.new_generation();
            }
            Ok(entry)
        }
    }

    fn formatted_source() -> Spreadsheet {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "original".into());
        sheet.set_cell_bold(0, 0, true);
        sheet
    }

    #[test]
    fn external_copy_supersedes_internal_snapshot() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.copy_selection(&mut clipboard);
        clipboard.replace_text("external\t");
        sheet.cursor_row = 1;
        sheet.set_cell(1, 1, "old".into());
        sheet.set_cell_bold(1, 1, true);

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(1, 0), "external");
        assert!(!sheet.get_cell_style(1, 0).bold);
        assert_eq!(sheet.get_cell(1, 1), "");
        assert!(sheet.get_cell_style(1, 1).bold);
        assert!(sheet.clipboard_data.is_none());
    }

    #[test]
    fn external_copy_with_identical_text_does_not_move_or_transfer_source_styles() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.replace_text("original");
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "original");
        assert!(!sheet.get_cell_style(0, 1).bold);
        assert!(sheet.clipboard_data.is_none());
    }

    #[test]
    fn history_replay_of_the_marked_payload_retires_cut_permanently() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.new_generation();

        for col in [1, 2] {
            sheet.cursor_col = col;
            sheet.paste(&mut clipboard);
            assert_eq!(sheet.get_cell(0, 0), "original");
            assert!(sheet.get_cell_style(0, 0).bold);
            assert_eq!(sheet.get_cell(0, col), "original");
            assert!(!sheet.get_cell_style(0, col).bold);
            assert!(sheet.clipboard_data.is_none());
        }
    }

    #[test]
    fn generation_change_during_read_cancels_cut_before_any_mutation() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.change_during_read = true;
        sheet.cursor_col = 1;
        sheet.set_cell(0, 1, "before paste".into());

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "before paste");
        assert!(sheet.clipboard_message.is_some());

        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "original");
        assert!(!sheet.get_cell_style(0, 1).bold);
    }

    #[test]
    fn generation_change_during_copy_read_leaves_destination_intact() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.copy_selection(&mut clipboard);
        clipboard.change_during_read = true;
        sheet.cursor_col = 1;
        sheet.set_cell(0, 1, "before paste".into());

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 1), "before paste");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn unchanged_publication_preserves_styles_and_allows_a_verified_move() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.copy_selection(&mut clipboard);
        sheet.cursor_col = 1;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert!(sheet.get_cell_style(0, 1).bold);

        sheet.cursor_col = 0;
        sheet.cut_selection(&mut clipboard);
        sheet.cursor_col = 2;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "");
        assert_eq!(sheet.get_cell(0, 2), "original");
        assert!(sheet.get_cell_style(0, 2).bold);
    }

    #[test]
    fn verified_cut_still_rechecks_the_source() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        sheet.set_cell(0, 0, "edited".into());
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "edited");
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn verified_cut_does_not_move_after_a_structural_edit() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        sheet.selected_rows = Some((1, 1));
        sheet.insert_rows_after_selected();
        sheet.cursor_row = 0;
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());
    }

    #[test]
    fn unverifiable_desktop_cut_becomes_styled_copy() {
        let mut clipboard = FakeClipboard::default();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());

        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "original");
        assert!(sheet.get_cell_style(0, 1).bold);
    }

    #[test]
    fn an_unavailable_generation_on_either_side_allows_only_styled_copy() {
        for (published, observed) in [(None, Some(1)), (Some(0), None)] {
            let mut clipboard = FakeClipboard {
                generation: published,
                ..FakeClipboard::default()
            };
            let mut sheet = formatted_source();
            sheet.cut_selection(&mut clipboard);
            clipboard.generation = observed;
            sheet.cursor_col = 1;

            sheet.paste(&mut clipboard);
            assert_eq!(sheet.get_cell(0, 0), "original");
            assert_eq!(sheet.get_cell(0, 1), "");
            assert!(sheet.clipboard_message.is_some());

            sheet.paste(&mut clipboard);
            assert_eq!(sheet.get_cell(0, 0), "original");
            assert_eq!(sheet.get_cell(0, 1), "original");
            assert!(sheet.get_cell_style(0, 1).bold);
        }
    }

    #[test]
    fn unreadable_desktop_entry_disarms_exported_cut() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.read_error = Some(AccessError::Unavailable);
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "");
        assert!(sheet.clipboard_message.is_some());

        clipboard.read_error = None;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "original");
        assert!(sheet.get_cell_style(0, 1).bold);
    }

    #[test]
    fn no_text_retires_existing_internal_snapshot() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.new_generation();
        clipboard.entry = None;
        sheet.cursor_col = 1;
        sheet.set_cell(0, 1, "before paste".into());

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "before paste");
        assert!(sheet.clipboard_data.is_none());
    }

    #[test]
    fn unavailable_desktop_still_allows_app_local_copy_and_cut() {
        let mut clipboard = OfflineClipboard;
        let mut sheet = formatted_source();
        sheet.copy_selection(&mut clipboard);
        sheet.cursor_col = 1;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "original");
        assert!(sheet.get_cell_style(0, 1).bold);

        sheet.cursor_col = 0;
        sheet.cut_selection(&mut clipboard);
        sheet.cursor_col = 2;
        sheet.paste(&mut clipboard);
        assert_eq!(sheet.get_cell(0, 0), "");
        assert_eq!(sheet.get_cell(0, 2), "original");
        assert!(sheet.get_cell_style(0, 2).bold);
    }

    #[test]
    fn failed_export_does_not_shadow_external_recovery() {
        let mut clipboard = FakeClipboard::tracked();
        clipboard.fail_write = true;
        let mut sheet = formatted_source();
        sheet.cut_selection(&mut clipboard);
        clipboard.replace_text("recovered");
        sheet.cursor_col = 1;

        sheet.paste(&mut clipboard);

        assert_eq!(sheet.get_cell(0, 0), "original");
        assert_eq!(sheet.get_cell(0, 1), "recovered");
        assert!(sheet.clipboard_data.is_none());
    }

    #[test]
    fn exported_plain_text_keeps_rectangular_tsv_without_the_marker() {
        let mut clipboard = FakeClipboard::tracked();
        let mut sheet = formatted_source();
        sheet.set_cell(1, 0, "café".into());
        sheet.selection_anchor = Some((0, 0));
        sheet.cursor_row = 1;
        sheet.cursor_col = 2;
        sheet.copy_selection(&mut clipboard);

        let entry = clipboard.entry.unwrap();
        assert_eq!(entry.text, "original\t\t\ncafé\t\t");
        assert!(entry.token.is_some());
    }
}
