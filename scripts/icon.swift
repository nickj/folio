import AppKit

let out = URL(fileURLWithPath: CommandLine.arguments[1])
let fm = FileManager.default
try fm.createDirectory(at: out, withIntermediateDirectories: true)
func rgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: r / 255, green: g / 255, blue: b / 255, alpha: a)
}
func render(_ pixels: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
    let tile = NSBezierPath(roundedRect: NSRect(x: 70, y: 70, width: 884, height: 884), xRadius: 202, yRadius: 202)
    NSGraphicsContext.saveGraphicsState()
    let tileShadow = NSShadow(); tileShadow.shadowColor = NSColor.black.withAlphaComponent(0.24); tileShadow.shadowBlurRadius = 34; tileShadow.shadowOffset = NSSize(width: 0, height: -12); tileShadow.set()
    rgb(18, 75, 67).setFill(); tile.fill()
    NSGraphicsContext.restoreGraphicsState()
    NSGradient(starting: rgb(49, 150, 126), ending: rgb(13, 61, 58))!.draw(in: tile, angle: -70)
    rgb(255, 255, 255, 0.18).setStroke(); tile.lineWidth = 2; tile.stroke()
    // A second leaf gives the icon its book-like depth.
    let back = NSBezierPath(roundedRect: NSRect(x: 256, y: 230, width: 506, height: 596), xRadius: 42, yRadius: 42)
    NSGraphicsContext.saveGraphicsState()
    context.cgContext.translateBy(x: 510, y: 510)
    context.cgContext.rotate(by: -8 * .pi / 180)
    context.cgContext.translateBy(x: -510, y: -510)
    rgb(157, 208, 185).setFill(); back.fill()
    NSGraphicsContext.restoreGraphicsState()
    let page = NSBezierPath()
    page.move(to: NSPoint(x: 308, y: 222)); page.line(to: NSPoint(x: 716, y: 222))
    page.curve(to: NSPoint(x: 758, y: 264), controlPoint1: NSPoint(x: 740, y: 222), controlPoint2: NSPoint(x: 758, y: 240))
    page.line(to: NSPoint(x: 758, y: 680)); page.line(to: NSPoint(x: 604, y: 834)); page.line(to: NSPoint(x: 308, y: 834))
    page.curve(to: NSPoint(x: 266, y: 792), controlPoint1: NSPoint(x: 284, y: 834), controlPoint2: NSPoint(x: 266, y: 816))
    page.line(to: NSPoint(x: 266, y: 264))
    page.curve(to: NSPoint(x: 308, y: 222), controlPoint1: NSPoint(x: 266, y: 240), controlPoint2: NSPoint(x: 284, y: 222)); page.close()
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow(); shadow.shadowColor = NSColor.black.withAlphaComponent(0.3); shadow.shadowBlurRadius = 28; shadow.shadowOffset = NSSize(width: 0, height: -14); shadow.set()
    rgb(250, 247, 232).setFill(); page.fill()
    NSGraphicsContext.restoreGraphicsState()
    let fold = NSBezierPath(); fold.move(to: NSPoint(x: 604, y: 834)); fold.line(to: NSPoint(x: 604, y: 718))
    fold.curve(to: NSPoint(x: 642, y: 680), controlPoint1: NSPoint(x: 604, y: 694), controlPoint2: NSPoint(x: 618, y: 680))
    fold.line(to: NSPoint(x: 758, y: 680)); fold.close(); rgb(212, 225, 203).setFill(); fold.fill()
    // The Markdown mark, drawn as vectors so every size is crisp.
    let mark = NSBezierPath()
    mark.move(to: NSPoint(x: 340, y: 440)); mark.line(to: NSPoint(x: 340, y: 584)); mark.line(to: NSPoint(x: 416, y: 510)); mark.line(to: NSPoint(x: 492, y: 584)); mark.line(to: NSPoint(x: 492, y: 440))
    mark.lineWidth = 38; mark.lineCapStyle = .square; mark.lineJoinStyle = .round; rgb(24, 81, 69).setStroke(); mark.stroke()
    let arrow = NSBezierPath(); arrow.move(to: NSPoint(x: 612, y: 594)); arrow.line(to: NSPoint(x: 612, y: 446)); arrow.move(to: NSPoint(x: 560, y: 492)); arrow.line(to: NSPoint(x: 612, y: 440)); arrow.line(to: NSPoint(x: 664, y: 492))
    arrow.lineWidth = 30; arrow.lineCapStyle = .round; arrow.lineJoinStyle = .round; arrow.stroke()
    let line = NSBezierPath(); line.move(to: NSPoint(x: 340, y: 338)); line.line(to: NSPoint(x: 584, y: 338)); line.lineWidth = 14; line.lineCapStyle = .round; rgb(175, 193, 169).setStroke(); line.stroke()
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}
for size in [16, 32, 128, 256, 512] {
    try render(size).write(to: out.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size * 2).write(to: out.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
