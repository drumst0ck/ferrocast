//! The `ferrocast://` URL scheme: strict parser plus an Apple Event (`GURL`) handler.
//!
//! Any web page or app can fire these URLs, so a URL can only navigate: it never carries text to
//! insert, commands or file paths, and everything that is not exactly one of the known shapes is
//! rejected.

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSAppleEventManager, NSObjectProtocol, NSString};

const SCHEME: &str = "ferrocast";
const HOST: &str = "notes";
const MAX_ID_LEN: usize = 64;
/// Longest URL looked at at all; anything longer cannot be a valid route.
const MAX_URL_LEN: usize = 128;

/// Where a `ferrocast://` URL points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// `ferrocast://notes/<id>`: open that note.
    Note(String),
    /// `ferrocast://notes/new`: create a note and open it.
    NewNote,
    /// `ferrocast://notes`: toggle the Notes window.
    Notes,
}

/// The Quicklink URL that opens the note `id`.
pub fn note_url(id: &str) -> String {
    format!("{SCHEME}://{HOST}/{id}")
}

fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Parse a `ferrocast://` URL. Returns `None` for anything that is not exactly a known route.
pub fn parse(url: &str) -> Option<Route> {
    if url.len() > MAX_URL_LEN {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return None;
    }
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    if !host.eq_ignore_ascii_case(HOST) {
        return None;
    }
    match path {
        None => Some(Route::Notes),
        // A trailing slash on the bare host is tolerated, nothing else after it.
        Some("") => Some(Route::Notes),
        Some(id) if id.eq_ignore_ascii_case("new") => Some(Route::NewNote),
        Some(id) if valid_id(id) => Some(Route::Note(id.to_string())),
        Some(_) => None,
    }
}

type UrlCb = Rc<dyn Fn(Route)>;

thread_local! {
    static ON_URL: RefCell<Option<UrlCb>> = const { RefCell::new(None) };
    static HANDLER: RefCell<Option<Retained<UrlHandler>>> = const { RefCell::new(None) };
    static WARNED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

define_class!(
    /// Target of the `GURL` Apple Event.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastUrlHandler"]
    struct UrlHandler;

    impl UrlHandler {
        #[unsafe(method(handleURLEvent:withReplyEvent:))]
        fn handle_url_event(&self, event: &AnyObject, _reply: Option<&AnyObject>) {
            dispatch(event_url(event).as_deref());
        }
    }

    unsafe impl NSObjectProtocol for UrlHandler {}
);

/// The direct-object string of a `GURL` event.
fn event_url(event: &AnyObject) -> Option<String> {
    // keyDirectObject = '----'
    const KEY_DIRECT_OBJECT: u32 = 0x2D2D_2D2D;
    let desc: Option<Retained<AnyObject>> = unsafe { msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT] };
    let s: Option<Retained<NSString>> = unsafe { msg_send![&*desc?, stringValue] };
    s.map(|s| s.to_string())
}

fn dispatch(url: Option<&str>) {
    match url.and_then(parse) {
        Some(route) => {
            let cb = ON_URL.with(|c| c.borrow().clone());
            if let Some(cb) = cb {
                cb(route);
            }
        }
        None => {
            if !WARNED.replace(true) {
                eprintln!("ferrocast: ignored an invalid ferrocast:// URL (further ones are ignored silently)");
            }
        }
    }
}

/// Register `on_url` for valid `ferrocast://` URLs; runs on the main thread. Safe to call before
/// the run loop starts (a URL that launched the app is delivered right after launch) and safe to
/// call again: the new closure replaces the previous one.
pub fn install(on_url: Box<dyn Fn(Route)>) {
    ON_URL.with(|c| *c.borrow_mut() = Some(Rc::from(on_url)));
    HANDLER.with(|h| {
        if h.borrow().is_some() {
            return;
        }
        let mtm = MainThreadMarker::new().expect("url_scheme::install must run on the main thread");
        let handler: Retained<UrlHandler> = unsafe { msg_send![mtm.alloc::<UrlHandler>(), init] };
        // kInternetEventClass = kAEGetURL = 'GURL'
        const GURL: u32 = 0x4755_524C;
        let manager = NSAppleEventManager::sharedAppleEventManager();
        // The typed binding needs the `objc2-core-services` feature of objc2-foundation, which this
        // crate does not enable, so send the message directly (AEEventClass / AEEventID are u32).
        unsafe {
            let _: () = msg_send![
                &*manager,
                setEventHandler: &*handler as &AnyObject,
                andSelector: sel!(handleURLEvent:withReplyEvent:),
                forEventClass: GURL,
                andEventID: GURL
            ];
        }
        *h.borrow_mut() = Some(handler);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "3f2b8c1e-9d4a-4e7b-8a61-0c5d2f9e7b14";

    #[test]
    fn note_url_round_trips() {
        assert_eq!(note_url(ID), format!("ferrocast://notes/{ID}"));
        assert_eq!(parse(&note_url(ID)), Some(Route::Note(ID.to_string())));
    }

    #[test]
    fn parses_new_note() {
        assert_eq!(parse("ferrocast://notes/new"), Some(Route::NewNote));
    }

    #[test]
    fn parses_notes_toggle() {
        assert_eq!(parse("ferrocast://notes"), Some(Route::Notes));
        assert_eq!(parse("ferrocast://notes/"), Some(Route::Notes));
    }

    #[test]
    fn scheme_and_host_are_case_insensitive() {
        assert_eq!(parse("FerroCast://NOTES/abc"), Some(Route::Note("abc".into())));
        assert_eq!(parse("FERROCAST://Notes"), Some(Route::Notes));
    }

    #[test]
    fn id_keeps_its_case() {
        assert_eq!(parse("ferrocast://notes/AbC-1"), Some(Route::Note("AbC-1".into())));
    }

    #[test]
    fn id_length_is_one_to_sixty_four() {
        let max = "a".repeat(64);
        assert_eq!(parse(&format!("ferrocast://notes/{max}")), Some(Route::Note(max)));
        assert_eq!(parse(&format!("ferrocast://notes/{}", "a".repeat(65))), None);
        assert_eq!(parse("ferrocast://notes/a"), Some(Route::Note("a".into())));
    }

    #[test]
    fn rejects_empty_id_segment() {
        assert_eq!(parse("ferrocast://notes//"), None);
        assert_eq!(parse("ferrocast://notes//abc"), None);
    }

    #[test]
    fn rejects_path_traversal() {
        assert_eq!(parse("ferrocast://notes/../x"), None);
        assert_eq!(parse("ferrocast://notes/.."), None);
        assert_eq!(parse("ferrocast://notes/.hidden"), None);
    }

    #[test]
    fn rejects_extra_segments() {
        assert_eq!(parse("ferrocast://notes/abc/def"), None);
        assert_eq!(parse("ferrocast://notes/abc/"), None);
        assert_eq!(parse("ferrocast://notes/new/x"), None);
    }

    #[test]
    fn rejects_query_and_fragment() {
        assert_eq!(parse("ferrocast://notes/abc?text=hi"), None);
        assert_eq!(parse("ferrocast://notes/abc#frag"), None);
        assert_eq!(parse("ferrocast://notes?x=1"), None);
        assert_eq!(parse("ferrocast://notes#x"), None);
        assert_eq!(parse("ferrocast://notes/new?x=1"), None);
    }

    #[test]
    fn rejects_other_hosts() {
        assert_eq!(parse("ferrocast://evil/abc"), None);
        assert_eq!(parse("ferrocast://notes.evil.com/abc"), None);
        assert_eq!(parse("ferrocast://user@notes/abc"), None);
        assert_eq!(parse("ferrocast://notes:80/abc"), None);
        assert_eq!(parse("ferrocast:///abc"), None);
    }

    #[test]
    fn rejects_other_schemes() {
        assert_eq!(parse("https://notes/abc"), None);
        assert_eq!(parse("ferrocast2://notes/abc"), None);
        assert_eq!(parse("file:///etc/passwd"), None);
        assert_eq!(parse("notes/abc"), None);
        assert_eq!(parse("ferrocast:notes/abc"), None);
    }

    #[test]
    fn rejects_percent_encoding() {
        assert_eq!(parse("ferrocast://notes/%2e%2e"), None);
        assert_eq!(parse("ferrocast://notes/a%20b"), None);
        assert_eq!(parse("ferrocast://notes/%6Eew"), None);
    }

    #[test]
    fn rejects_other_characters_and_whitespace() {
        assert_eq!(parse("ferrocast://notes/a_b"), None);
        assert_eq!(parse("ferrocast://notes/a b"), None);
        assert_eq!(parse("ferrocast://notes/abc\n"), None);
        assert_eq!(parse("ferrocast://notes/é"), None);
        assert_eq!(parse(" ferrocast://notes/abc"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn rejects_very_long_input() {
        assert_eq!(parse(&format!("ferrocast://notes/{}", "a".repeat(10_000))), None);
    }
}
