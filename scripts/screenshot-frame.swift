// Present Rune's actual 2× render on a quiet backdrop. The --chrome render also contains
// diagnostic bounds; copy only its traffic-light pixels, leaving those bounds out of docs.
import AppKit

let args = CommandLine.arguments
let raw = NSImage(contentsOfFile: args[1])!
let chrome = NSImage(contentsOfFile: args[2])!
let bitmap = NSBitmapImageRep(data: try Data(contentsOf: URL(fileURLWithPath: args[1])))!
let width = CGFloat(bitmap.pixelsWide), height = CGFloat(bitmap.pixelsHigh)
let pad: CGFloat = 64
let dark = args[4] == "dark"
func rgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat) -> NSColor {
    NSColor(srgbRed: r / 255, green: g / 255, blue: b / 255, alpha: 1)
}
let output = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(width + pad * 2), pixelsHigh: Int(height + pad * 2), bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: output)!
let bounds = NSRect(x: 0, y: 0, width: width + pad * 2, height: height + pad * 2)
NSGradient(starting: dark ? rgb(31, 44, 44) : rgb(240, 237, 228), ending: dark ? rgb(16, 24, 28) : rgb(222, 234, 228))!.draw(in: bounds, angle: -45)
let window = NSRect(x: pad, y: pad, width: width, height: height)
let outline = NSBezierPath(roundedRect: window, xRadius: 22, yRadius: 22)
NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(dark ? 0.4 : 0.2)
shadow.shadowBlurRadius = 36
shadow.shadowOffset = NSSize(width: 0, height: -12)
shadow.set()
NSColor.black.setFill(); outline.fill()
NSGraphicsContext.restoreGraphicsState()
NSGraphicsContext.saveGraphicsState()
outline.addClip()
raw.draw(in: window, from: .zero, operation: .copy, fraction: 1)
// Chrome's fixed 18/26 pt origin, 14 pt discs and 23 pt pitch, rendered at 2×.
let crop = chrome.cgImage(forProposedRect: nil, context: nil, hints: nil)!.cropping(to: CGRect(x: 34, y: 50, width: 124, height: 32))!
NSImage(cgImage: crop, size: NSSize(width: 124, height: 32)).draw(in: NSRect(x: pad + 34, y: pad + height - 82, width: 124, height: 32))
NSGraphicsContext.restoreGraphicsState()
NSColor.white.withAlphaComponent(dark ? 0.12 : 0.7).setStroke(); outline.lineWidth = 1; outline.stroke()
NSGraphicsContext.restoreGraphicsState()
try output.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[3]))
