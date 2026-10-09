// Prints the CGWindowID of the frontmost on-screen window owned by a process whose name matches argv[1].
import Cocoa
let name = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "OptChat"
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
for w in list {
    if let owner = w[kCGWindowOwnerName as String] as? String, owner == name, let layer = w[kCGWindowLayer as String] as? Int, layer == 0 {
        if let id = w[kCGWindowNumber as String] as? Int { print(id); exit(0) }
    }
}
exit(1)
