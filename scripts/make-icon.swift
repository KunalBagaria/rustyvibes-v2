// Renders the Rustyvibes app icon.
//   swift scripts/make-icon.swift assets/icon
// Writes AppIcon.iconset/*.png, AppIcon.icns (via iconutil) and AppIcon-1024.png.

import AppKit

let outDir = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "assets/icon"
let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!

func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(
        srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
        green: CGFloat((hex >> 8) & 0xFF) / 255,
        blue: CGFloat(hex & 0xFF) / 255,
        alpha: alpha)
}

func gradient(_ colors: [CGColor], _ locations: [CGFloat]) -> CGGradient {
    CGGradient(colorsSpace: sRGB, colors: colors as CFArray, locations: locations)!
}

/// Draws the icon into a 1024×1024 coordinate space.
func drawIcon(_ cg: CGContext) {
    // The macOS icon grid: an 824 pt rounded square centred in the canvas.
    let body = CGRect(x: 100, y: 100, width: 824, height: 824)
    let tile = CGPath(roundedRect: body, cornerWidth: 186, cornerHeight: 186, transform: nil)

    cg.saveGState()
    cg.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: rgb(0x000000, 0.32))
    cg.addPath(tile)
    cg.setFillColor(rgb(0xC0461B))
    cg.fillPath()
    cg.restoreGState()

    cg.saveGState()
    cg.addPath(tile)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xF68A4B), rgb(0xE0602A), rgb(0xA9330D)], [0, 0.5, 1]),
        start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 100), options: [])
    cg.drawRadialGradient(
        gradient([rgb(0xFFFFFF, 0.25), rgb(0xFFFFFF, 0)], [0, 1]),
        startCenter: CGPoint(x: 512, y: 960), startRadius: 0,
        endCenter: CGPoint(x: 512, y: 960), endRadius: 640, options: [])

    // Sound waves either side of the key.
    cg.setLineCap(.round)
    for (radius, alpha) in [(CGFloat(262), CGFloat(0.9)), (CGFloat(334), CGFloat(0.5))] {
        cg.setStrokeColor(rgb(0xFFF3E4, alpha))
        cg.setLineWidth(30)
        for side in [CGFloat(0), CGFloat.pi] {
            cg.addArc(center: CGPoint(x: 512, y: 486), radius: radius,
                      startAngle: side - 0.40, endAngle: side + 0.40, clockwise: false)
            cg.strokePath()
        }
    }

    // Keycap: soft shadow, skirt, then the dished top.
    let skirt = CGPath(roundedRect: CGRect(x: 327, y: 300, width: 370, height: 362),
                       cornerWidth: 72, cornerHeight: 72, transform: nil)
    cg.saveGState()
    cg.setShadow(offset: CGSize(width: 0, height: -22), blur: 34, color: rgb(0x3A0E00, 0.45))
    cg.addPath(skirt)
    cg.setFillColor(rgb(0xE6CFA9))
    cg.fillPath()
    cg.restoreGState()

    cg.saveGState()
    cg.addPath(skirt)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xF4E4C8), rgb(0xD9BE92)], [0, 1]),
        start: CGPoint(x: 512, y: 662), end: CGPoint(x: 512, y: 300), options: [])
    cg.restoreGState()

    let top = CGPath(roundedRect: CGRect(x: 377, y: 398, width: 270, height: 238),
                     cornerWidth: 46, cornerHeight: 46, transform: nil)
    cg.saveGState()
    cg.addPath(top)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xFFFAF0), rgb(0xF2E2C4)], [0, 1]),
        start: CGPoint(x: 512, y: 636), end: CGPoint(x: 512, y: 398), options: [])
    // The dish: a gentle darker centre.
    cg.drawRadialGradient(
        gradient([rgb(0xC9A979, 0.22), rgb(0xC9A979, 0)], [0, 1]),
        startCenter: CGPoint(x: 512, y: 510), startRadius: 0,
        endCenter: CGPoint(x: 512, y: 510), endRadius: 150, options: [])
    cg.restoreGState()

    cg.restoreGState()
}

func render(_ pixels: Int) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
    drawIcon(context.cgContext)
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func writePNG(_ rep: NSBitmapImageRep, _ path: String) {
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

let iconset = "\(outDir)/AppIcon.iconset"
try? FileManager.default.removeItem(atPath: iconset)
try! FileManager.default.createDirectory(atPath: iconset, withIntermediateDirectories: true)
for (name, pixels) in [
    ("icon_16x16", 16), ("icon_16x16@2x", 32), ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256), ("icon_256x256", 256),
    ("icon_256x256@2x", 512), ("icon_512x512", 512), ("icon_512x512@2x", 1024),
] {
    writePNG(render(pixels), "\(iconset)/\(name).png")
}
writePNG(render(1024), "\(outDir)/AppIcon-1024.png")

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset, "-o", "\(outDir)/AppIcon.icns"]
try! iconutil.run()
iconutil.waitUntilExit()
try? FileManager.default.removeItem(atPath: iconset)
print("wrote \(outDir)/AppIcon.icns and \(outDir)/AppIcon-1024.png")
