// ferrocast-translate: Apple Translation for Ferrocast.
//
// One JSON object on stdin, one JSON object on stdout (see crates/core/src/apple_translate.rs).
//   {"cmd":"status","source":"en","target":"de"}          -> {"status":"installed|supported|unsupported"}
//   {"cmd":"translate","source":"en","target":"de","text":"..."} -> {"text":"..."} or {"error":"..."}
// "source" may be missing: the system language is used for status, the text's language for translate.
// Exit codes: 0 answered, 1 error, 2 macOS older than 26 (headless sessions need it).
// The Translation framework is weak-linked, so this binary also starts on older macOS.

import Foundation
import NaturalLanguage
import Translation

func reply(_ object: [String: Any], code: Int32 = 0) -> Never {
  let data = (try? JSONSerialization.data(withJSONObject: object, options: [.withoutEscapingSlashes])) ?? Data("{\"error\":\"encode\"}".utf8)
  FileHandle.standardOutput.write(data)
  FileHandle.standardOutput.write(Data("\n".utf8))
  exit(code)
}

func fail(_ message: String, language: String? = nil, code: Int32 = 1) -> Never {
  var o: [String: Any] = ["error": message]
  if let language { o["language"] = language }
  reply(o, code: code)
}

@available(macOS 26.0, *)
func run(_ request: [String: Any]) async -> Never {
  guard let cmd = request["cmd"] as? String, let targetCode = request["target"] as? String else { fail("bad request") }
  let target = Locale.Language(identifier: targetCode)
  let availability = LanguageAvailability()

  func source(for text: String?) -> Locale.Language? {
    if let s = request["source"] as? String, !s.isEmpty { return Locale.Language(identifier: s) }
    if let text {
      let recognizer = NLLanguageRecognizer()
      recognizer.processString(text)
      if let l = recognizer.dominantLanguage { return Locale.Language(identifier: l.rawValue) }
      return nil
    }
    return Locale.current.language
  }

  switch cmd {
  case "status":
    guard let src = source(for: nil) else { fail("no source language") }
    switch await availability.status(from: src, to: target) {
    case .installed: reply(["status": "installed"])
    case .supported: reply(["status": "supported"])
    default: reply(["status": "unsupported"])
    }
  case "translate":
    guard let text = request["text"] as? String, !text.isEmpty else { fail("empty text") }
    guard let src = source(for: text) else { fail("could not tell the language of the text") }
    let srcCode = src.minimalIdentifier
    if src.languageCode == target.languageCode && src.script == target.script {
      reply(["text": text])  // already in the target language
    }
    switch await availability.status(from: src, to: target) {
    case .unsupported: fail("unsupported", language: targetCode)
    case .supported:
      // one of the two packs is missing; name the one the user can act on
      fail("needs-download", language: targetCode)
    default: break
    }
    do {
      let session = TranslationSession(installedSource: src, target: target)
      let response = try await session.translate(text)
      reply(["text": response.targetText])
    } catch {
      fail("\(srcCode) → \(targetCode): \(error.localizedDescription)")
    }
  default:
    fail("unknown command")
  }
}

@main struct Main {
  static func main() async {
    let input = FileHandle.standardInput.readDataToEndOfFile()
    guard let request = (try? JSONSerialization.jsonObject(with: input)) as? [String: Any] else { fail("bad request") }
    if #available(macOS 26.0, *) {
      await run(request)
    } else {
      fail("unsupported-os", code: 2)
    }
  }
}
