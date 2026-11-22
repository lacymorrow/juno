// usage: swift check-png.swift <png>
// Exits 0 only when the image has real content. A blocked Screen Recording
// capture is a blank or flat image, so we require a minimum image size, enough
// distinct colors, and that no single color covers nearly everything.
import CoreGraphics
import Foundation
import ImageIO

guard CommandLine.arguments.count > 1,
      let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL, nil),
      let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { print("FAIL: unreadable png"); exit(1) }
let w = img.width, h = img.height
guard w >= 300, h >= 200 else { print("FAIL: image too small \(w)x\(h)"); exit(1) }
var px = [UInt8](repeating: 0, count: w * h * 4)
let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                    space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
var counts = [UInt32: Int]()
for i in stride(from: 0, to: px.count, by: 4) {
    let k = UInt32(px[i] >> 3) << 16 | UInt32(px[i+1] >> 3) << 8 | UInt32(px[i+2] >> 3)
    counts[k, default: 0] += 1
}
let top = Double(counts.values.max() ?? 0) / Double(w * h)
print("size=\(w)x\(h) distinct=\(counts.count) dominant=\(String(format: "%.3f", top))")
if counts.count < 12 || top > 0.97 { print("FAIL: image is uniform or near-uniform"); exit(1) }
print("OK")
