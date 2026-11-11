import AppKit
import CoreServices
import UniformTypeIdentifiers

let bundle = "com.nickjensen.folio" as CFString
let extensions = ["md", "markdown", "mdown", "mkd", "mkdn"]
var types = Set(["net.daringfireball.markdown"])
for ext in extensions {
    if let type = UTType(filenameExtension: ext) { types.insert(type.identifier) }
}
for type in types.sorted() {
    let status = LSSetDefaultRoleHandlerForContentType(type as CFString, .all, bundle)
    guard status == noErr else {
        fputs("Couldn’t set the default for \(type): \(status)\n", stderr)
        exit(1)
    }
}
let samples = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString)
try FileManager.default.createDirectory(at: samples, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: samples) }
for ext in extensions {
    let sample = samples.appendingPathComponent("check.\(ext)")
    try Data("# Folio".utf8).write(to: sample)
    let handler = NSWorkspace.shared.urlForApplication(toOpen: sample)
    print(".\(ext): \(handler?.path ?? "no handler")")
    guard handler?.lastPathComponent == "Folio.app" else { exit(1) }
}
