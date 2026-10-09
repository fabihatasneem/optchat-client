// Prints the CGWindowID of the on-screen window owned by process id argv[1] (only our own process, never another app).
import Cocoa
guard CommandLine.arguments.count > 1, let pid = Int(CommandLine.arguments[1]) else { exit(2) }
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
for w in list {
    if let owner = w[kCGWindowOwnerPID as String] as? Int, owner == pid, let layer = w[kCGWindowLayer as String] as? Int, layer == 0,
       let id = w[kCGWindowNumber as String] as? Int { print(id); exit(0) }
}
exit(1)
