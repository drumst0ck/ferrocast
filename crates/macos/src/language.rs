//! On-device language detection (Apple's NaturalLanguage framework): nothing leaves the Mac. Used
//! to check that a translation came out in the language the user picked.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

#[link(name = "NaturalLanguage", kind = "framework")]
extern "C" {}

/// The most likely language of `text` as a BCP 47 code ("de", "es", "zh-Hans") and its confidence
/// (0..1), or `None` for empty text or when the framework is unavailable. Safe off the main thread.
pub fn detect(text: &str) -> Option<(String, f64)> {
    if text.trim().is_empty() {
        return None;
    }
    objc2::rc::autoreleasepool(|_| {
        let cls = AnyClass::get(c"NLLanguageRecognizer")?;
        let recognizer: Option<Retained<AnyObject>> = unsafe { msg_send![cls, new] };
        let recognizer = recognizer?;
        let s = NSString::from_str(text);
        unsafe {
            let _: () = msg_send![&*recognizer, processString: &*s];
        }
        let hypotheses: Option<Retained<NSDictionary<NSString, NSNumber>>> =
            unsafe { msg_send![&*recognizer, languageHypothesesWithMaximum: 1usize] };
        let hypotheses = hypotheses?;
        let keys: Retained<NSArray<NSString>> = hypotheses.allKeys();
        let key = keys.firstObject()?;
        let p = hypotheses.objectForKey(&key)?.doubleValue();
        Some((key.to_string(), p))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_german_and_a_spanish_sentence_confidently() {
        let (code, p) = detect("Übersetze dies stattdessen ins Deutsche.").expect("German detected");
        assert_eq!(code, "de");
        assert!(p >= 0.8, "{p}");
        let (code, _) = detect("La reunión se trasladó al jueves porque la mitad del equipo está de viaje.").unwrap();
        assert_eq!(code, "es");
    }

    #[test]
    fn empty_text_has_no_language() {
        assert_eq!(detect(""), None);
        assert_eq!(detect("   \n"), None);
    }
}
