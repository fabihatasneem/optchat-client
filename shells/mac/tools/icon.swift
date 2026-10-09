// Renders the OptChat logo (a rounded square with an indigo→violet gradient and a soft glow) as a 1024 px PNG.
import Cocoa
let size = 1024.0
let img = NSImage(size: NSSize(width: size, height: size))
img.lockFocus()
let ctx = NSGraphicsContext.current!.cgContext
let inset = 100.0
let rect = CGRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let path = CGPath(roundedRect: rect, cornerWidth: 190, cornerHeight: 190, transform: nil)
ctx.setShadow(offset: .zero, blur: 60, color: NSColor(calibratedRed: 0.56, green: 0.64, blue: 1, alpha: 0.55).cgColor)
ctx.addPath(path); ctx.setFillColor(NSColor.black.cgColor); ctx.fillPath()
ctx.setShadow(offset: .zero, blur: 0, color: nil)
ctx.addPath(path); ctx.clip()
let colors = [NSColor(calibratedRed: 0x8e/255.0, green: 0xa2/255.0, blue: 1, alpha: 1).cgColor, NSColor(calibratedRed: 0xc4/255.0, green: 0x9b/255.0, blue: 1, alpha: 1).cgColor, NSColor(calibratedRed: 0x5c/255.0, green: 0xc8/255.0, blue: 0xe0/255.0, alpha: 1).cgColor] as CFArray
let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 0.6, 1])!
ctx.drawLinearGradient(g, start: CGPoint(x: inset, y: size - inset), end: CGPoint(x: size - inset, y: inset), options: [])
// a chat bubble mark
ctx.setFillColor(NSColor(white: 1, alpha: 0.92).cgColor)
let b = CGPath(roundedRect: CGRect(x: 300, y: 380, width: 424, height: 290), cornerWidth: 90, cornerHeight: 90, transform: nil)
ctx.addPath(b); ctx.fillPath()
ctx.move(to: CGPoint(x: 360, y: 400)); ctx.addLine(to: CGPoint(x: 330, y: 300)); ctx.addLine(to: CGPoint(x: 450, y: 390)); ctx.fillPath()
img.unlockFocus()
let rep = NSBitmapImageRep(data: img.tiffRepresentation!)!
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
